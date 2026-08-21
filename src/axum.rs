//! Axum integration for Datastar.

use {
    crate::{
        DatalineWriter,
        consts::{self, DATASTAR_REQ_HEADER_STR},
        prelude::{DatastarEvent, ExecuteScript, PatchElements, PatchSignals},
    },
    axum::{
        Json,
        body::Bytes,
        extract::{FromRequest, OptionalFromRequest, Query, Request},
        http::{self},
        response::{
            IntoResponse, Response,
            sse::{Event, EventDataWriter},
        },
    },
    core::time::Duration,
    serde::{Deserialize, de::DeserializeOwned},
    std::fmt::{self, Write},
};

struct AxumDatalineWriter {
    writer: EventDataWriter,
    wrote_dataline: bool,
}

impl AxumDatalineWriter {
    fn new(writer: EventDataWriter) -> Self {
        Self {
            writer,
            wrote_dataline: false,
        }
    }

    fn into_event(self) -> Event {
        self.writer.into_event()
    }
}

impl DatalineWriter for AxumDatalineWriter {
    fn write_dataline(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        if std::mem::replace(&mut self.wrote_dataline, true) {
            self.writer.write_char('\n')?;
        }
        self.writer.write_fmt(args)
    }
}

fn write_axum_event(
    event_type: consts::EventType,
    id: Option<&str>,
    retry: Duration,
    write_datalines: impl FnOnce(&mut AxumDatalineWriter) -> fmt::Result,
) -> Event {
    let event = Event::default().event(event_type.as_str());
    let event = if retry.as_millis() != (consts::DEFAULT_SSE_RETRY_DURATION as u128) {
        event.retry(retry)
    } else {
        event
    };
    let event = match id {
        Some(id) => event.id(id),
        None => event,
    };

    let mut writer = AxumDatalineWriter::new(event.into_data_writer());
    write_datalines(&mut writer).expect("Axum's EventDataWriter is infallible");
    writer.into_event()
}

impl PatchElements {
    /// Write this [`PatchElements`] into an Axum SSE [`Event`].
    pub fn write_as_axum_sse_event(&self) -> Event {
        write_axum_event(
            consts::EventType::PatchElements,
            self.id.as_deref(),
            self.retry,
            |writer| self.write_datalines(writer),
        )
    }
}

impl From<PatchElements> for Event {
    fn from(value: PatchElements) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl From<&PatchElements> for Event {
    fn from(value: &PatchElements) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl PatchSignals {
    /// Write this [`PatchSignals`] into an Axum SSE [`Event`].
    pub fn write_as_axum_sse_event(&self) -> Event {
        write_axum_event(
            consts::EventType::PatchSignals,
            self.id.as_deref(),
            self.retry,
            |writer| self.write_datalines(writer),
        )
    }
}

impl From<PatchSignals> for Event {
    fn from(value: PatchSignals) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl From<&PatchSignals> for Event {
    fn from(value: &PatchSignals) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl ExecuteScript {
    /// Write this [`ExecuteScript`] into an Axum SSE [`Event`].
    pub fn write_as_axum_sse_event(&self) -> Event {
        write_axum_event(
            consts::EventType::PatchElements,
            self.id.as_deref(),
            self.retry,
            |writer| self.write_datalines(writer),
        )
    }
}

impl From<ExecuteScript> for Event {
    fn from(value: ExecuteScript) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl From<&ExecuteScript> for Event {
    fn from(value: &ExecuteScript) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl DatastarEvent {
    /// Turn this [`DatastarEvent`] into an Axum SSE [`Event`].
    pub fn write_as_axum_sse_event(&self) -> Event {
        write_axum_event(self.event, self.id.as_deref(), self.retry, |writer| {
            for line in &self.data {
                writer.write_dataline(format_args!("{line}"))?;
            }
            Ok(())
        })
    }
}

impl From<DatastarEvent> for Event {
    fn from(value: DatastarEvent) -> Self {
        value.write_as_axum_sse_event()
    }
}

impl From<&DatastarEvent> for Event {
    fn from(value: &DatastarEvent) -> Self {
        value.write_as_axum_sse_event()
    }
}

#[derive(Deserialize)]
struct DatastarParam {
    datastar: serde_json::Value,
}

/// [`ReadSignals`] is a request extractor that reads datastar signals from the request.
///
/// # Examples
///
/// ```
/// use datastar::axum::ReadSignals;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Signals {
///     foo: String,
///     bar: i32,
/// }
///
/// async fn handler(ReadSignals(signals): ReadSignals<Signals>) {
///    println!("foo: {}", signals.foo);
///    println!("bar: {}", signals.bar);
/// }
///
/// ```
#[derive(Debug)]
pub struct ReadSignals<T: DeserializeOwned>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> OptionalFromRequest<S> for ReadSignals<T>
where
    Bytes: FromRequest<S>,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        if req.headers().get(DATASTAR_REQ_HEADER_STR).is_none() {
            return Ok(None);
        }
        Ok(Some(
            <Self as FromRequest<S>>::from_request(req, state).await?,
        ))
    }
}

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ReadSignals<T>
where
    Bytes: FromRequest<S>,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let json = match *req.method() {
            http::Method::GET => {
                let query = Query::<DatastarParam>::from_request(req, state)
                    .await
                    .map_err(IntoResponse::into_response)?;

                let signals = query.0.datastar.as_str().ok_or(
                    (http::StatusCode::BAD_REQUEST, "Failed to parse JSON str").into_response(),
                )?;

                serde_json::from_str(signals).map_err(
                    #[cfg_attr(not(feature = "tracing"), expect(unused_variables))]
                    |err| {
                        #[cfg(feature = "tracing")]
                        tracing::debug!(%err, "failed to parse JSON value");

                        (
                            http::StatusCode::BAD_REQUEST,
                            "Failed to parse JSON value from query",
                        )
                            .into_response()
                    },
                )
            }
            _ => {
                let Json(json) = <Json<T> as FromRequest<S>>::from_request(req, state)
                    .await
                    .map_err(
                        #[cfg_attr(not(feature = "tracing"), expect(unused_variables))]
                        |err| {
                            #[cfg(feature = "tracing")]
                            tracing::debug!(%err, "failed to parse JSON value from payload");

                            (
                                http::StatusCode::BAD_REQUEST,
                                "Failed to parse JSON value from payload",
                            )
                                .into_response()
                        },
                    )?;
                Ok(json)
            }
        }?;
        Ok(Self(json))
    }
}

/// Datastar's headers
pub mod header {
    use {
        crate::consts::{ElementPatchMode, Namespace},
        axum::http::{HeaderName, HeaderValue},
    };

    /// A CSS selector for the target elements to patch
    pub const DATASTAR_SELECTOR: HeaderName = HeaderName::from_static("datastar-selector");

    /// How to patch the elements (See [`ElementPatchMode`]). Defaults to [`ElementPatchMode::Outer`].
    pub const DATASTAR_MODE: HeaderName = HeaderName::from_static("datastar-mode");

    /// Whether to use the [View Transition API](https://developer.mozilla.org/en-US/docs/Web/API/View_Transition_API) when patching elements.
    pub const DATASTAR_USE_VIEW_TRANSITION: HeaderName =
        HeaderName::from_static("datastar-use-view-transition");

    /// The namespace in which elements are created.
    pub const DATASTAR_NAMESPACE: HeaderName = HeaderName::from_static("datastar-namespace");

    /// If set to true, only patch signals that don’t already exist
    pub const DATASTAR_ONLY_IF_MISSING: HeaderName =
        HeaderName::from_static("datastar-only-if-missing");

    /// Sets the script element’s attributes using a JSON encoded string.
    pub const DATASTAR_SCRIPT_ATTRIBUTES: HeaderName =
        HeaderName::from_static("datastar-script-attributes");

    impl From<ElementPatchMode> for HeaderValue {
        fn from(value: ElementPatchMode) -> Self {
            HeaderValue::from_static(value.as_str())
        }
    }

    impl From<Namespace> for HeaderValue {
        fn from(value: Namespace) -> Self {
            HeaderValue::from_static(value.as_str())
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::consts::{ElementPatchMode, Namespace},
        axum::{
            body::to_bytes,
            response::{IntoResponse, Sse},
        },
        core::convert::Infallible,
        tokio_stream::iter,
    };

    async fn render(event: Event) -> String {
        let response = Sse::new(iter([Ok::<_, Infallible>(event)])).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn writes_patch_elements_directly() {
        let event = PatchElements::new("<circle id=\"dot\" />\n<path />")
            .id("elements-1")
            .retry(Duration::from_millis(2_000))
            .selector("#vis")
            .mode(ElementPatchMode::Append)
            .use_view_transition(true)
            .view_transition_selector("#main")
            .namespace(Namespace::Svg);

        assert_eq!(
            render(event.write_as_axum_sse_event()).await,
            concat!(
                "event: datastar-patch-elements\n",
                "retry: 2000\n",
                "id: elements-1\n",
                "data: selector #vis\n",
                "data: mode append\n",
                "data: useViewTransition true\n",
                "data: viewTransitionSelector #main\n",
                "data: namespace svg\n",
                "data: elements <circle id=\"dot\" />\n",
                "data: elements <path />\n\n",
            )
        );
    }

    #[tokio::test]
    async fn writes_patch_signals_directly() {
        let event = PatchSignals::new("{foo: 1,\nbar: 2}").only_if_missing(true);

        assert_eq!(
            render(event.write_as_axum_sse_event()).await,
            concat!(
                "event: datastar-patch-signals\n",
                "data: onlyIfMissing true\n",
                "data: signals {foo: 1,\n",
                "data: signals bar: 2}\n\n",
            )
        );
    }

    #[tokio::test]
    async fn writes_execute_script_directly() {
        let event = ExecuteScript::new("console.log('one')\nconsole.log('two')")
            .auto_remove(false)
            .attributes([r#"type="module""#, "defer"]);

        assert_eq!(
            render(event.write_as_axum_sse_event()).await,
            concat!(
                "event: datastar-patch-elements\n",
                "data: selector body\n",
                "data: mode append\n",
                "data: elements <script type=\"module\" defer>console.log('one')\n",
                "data: elements console.log('two')</script>\n\n",
            )
        );
    }

    #[tokio::test]
    async fn writes_generic_events_without_joining_data() {
        let event = PatchElements::new("<div>one</div>\n<div>two</div>").as_datastar_event();

        assert_eq!(
            render(event.write_as_axum_sse_event()).await,
            concat!(
                "event: datastar-patch-elements\n",
                "data: elements <div>one</div>\n",
                "data: elements <div>two</div>\n\n",
            )
        );
    }
}
