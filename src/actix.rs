//! Actix-Web integration for Datastar

use {
    crate::prelude::{DatastarEvent, ExecuteScript, PatchElements, PatchSignals},
    actix_web::{
        FromRequest, HttpRequest, HttpResponse, Responder, body::BoxBody, dev::Payload, error,
        http::Method, web,
    },
    bytes::Bytes,
    futures_util::{Stream, StreamExt, stream},
    serde::{Deserialize, de::DeserializeOwned},
    std::{convert::Infallible, future::Future, pin::Pin},
};

/// Write this [`PatchElements`] into an Actix-Web [`Sse`] streaming response.
///
/// This consumes the [`PatchElements`] and returns a single-event SSE response. For
/// streaming multiple events, use [`Sse::new`] with a stream of [`DatastarEvent`]s.
impl From<PatchElements> for Sse {
    fn from(value: PatchElements) -> Self {
        value.as_datastar_event().into()
    }
}

/// Write this [`PatchSignals`] into an Actix-Web [`Sse`] streaming response.
///
/// This consumes the [`PatchSignals`] and returns a single-event SSE response. For
/// streaming multiple events, use [`Sse::new`] with a stream of [`DatastarEvent`]s.
impl From<PatchSignals> for Sse {
    fn from(value: PatchSignals) -> Self {
        value.as_datastar_event().into()
    }
}

/// Write this [`ExecuteScript`] into an Actix-Web [`Sse`] streaming response.
///
/// This consumes the [`ExecuteScript`] and returns a single-event SSE response. For
/// streaming multiple events, use [`Sse::new`] with a stream of [`DatastarEvent`]s.
impl From<ExecuteScript> for Sse {
    fn from(value: ExecuteScript) -> Self {
        value.as_datastar_event().into()
    }
}

/// Convert a [`DatastarEvent`] into a single-event [`Sse`] streaming response.
///
/// For streaming multiple events, use [`Sse::new`] with a stream of [`DatastarEvent`]s.
impl From<DatastarEvent> for Sse {
    fn from(event: DatastarEvent) -> Self {
        Sse::new(stream::iter([event]))
    }
}

/// A streaming Server-Sent Events (SSE) response wrapping a stream of [`DatastarEvent`]s.
///
/// This is the core response type for the Actix-Web integration. Each [`DatastarEvent`]
/// in the stream is serialized to the SSE wire format via its [`std::fmt::Display`]
/// implementation and flushed to the client as it arrives.
///
/// For single-event responses, any builder type ([`PatchElements`], [`PatchSignals`],
/// [`ExecuteScript`]) or [`DatastarEvent`] can be converted into an `Sse` via [`From`].
///
/// # Examples
///
/// ## Single event
///
/// ```no_run
/// use actix_web::{get, Responder};
/// use datastar::prelude::PatchElements;
/// use datastar::actix::Sse;
///
/// #[get("/update")]
/// async fn update() -> impl Responder {
///     Sse::from(PatchElements::new("<div>Hello!</div>"))
/// }
/// ```
///
/// ## Streaming multiple events
///
/// ```no_run
/// use actix_web::{get, Responder};
/// use datastar::actix::Sse;
/// use datastar::{consts, prelude::DatastarEvent};
/// use futures_util::stream;
/// use std::time::Duration;
///
/// #[get("/progress")]
/// async fn progress() -> impl Responder {
///     let events = stream::iter([
///         DatastarEvent {
///             event: consts::EventType::PatchElements,
///             id: None,
///             retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
///             data: vec!["step 1".to_string()],
///         },
///         DatastarEvent {
///             event: consts::EventType::PatchElements,
///             id: None,
///             retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
///             data: vec!["step 2".to_string()],
///         },
///     ]);
///     Sse::new(events)
/// }
/// ```
pub struct Sse {
    stream: Pin<Box<dyn Stream<Item = DatastarEvent>>>,
}

impl std::fmt::Debug for Sse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sse").finish_non_exhaustive()
    }
}

impl Sse {
    /// Create a new [`Sse`] response from a stream of [`DatastarEvent`]s.
    ///
    /// The stream is not required to be [`Send`]: Actix-Web runs handlers on the
    /// worker thread rather than moving them between threads, so its own
    /// `Handler::Future` and `HttpResponse::streaming` bounds are `!Send` too.
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = DatastarEvent> + 'static,
    {
        Self {
            stream: Box::pin(stream),
        }
    }
}

impl Responder for Sse {
    type Body = BoxBody;

    fn respond_to(self, _: &HttpRequest) -> HttpResponse<Self::Body> {
        let stream = self
            .stream
            .map(|event| Ok::<Bytes, Infallible>(Bytes::from(event.to_string())));

        HttpResponse::Ok()
            .content_type("text/event-stream")
            .insert_header(("Cache-Control", "no-cache"))
            .streaming(stream)
    }
}

#[derive(Deserialize)]
struct DatastarParam {
    datastar: Option<serde_json::Value>,
}

/// [`ReadSignals`] is a request extractor that reads datastar signals from the request.
///
/// # Examples
///
/// ```
/// use datastar::actix::ReadSignals;
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

impl<T: DeserializeOwned + 'static> FromRequest for ReadSignals<T> {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let req = req.clone();
        // Datastar sends signals in the `datastar` query parameter for GET and
        // DELETE, and in the JSON body for every other method (including QUERY).
        let is_query = matches!(*req.method(), Method::GET | Method::DELETE);

        if is_query {
            let query_fut = web::Query::<DatastarParam>::from_request(&req, payload);
            Box::pin(async move {
                let query = query_fut.await?;
                let signals = match query.0.datastar.as_ref() {
                    Some(value) => value
                        .as_str()
                        .ok_or_else(|| error::ErrorBadRequest("Failed to parse JSON str"))?,
                    // No `datastar` parameter: deserialize `null` so that
                    // `ReadSignals<Option<T>>` yields `None` on a full page load.
                    None => "null",
                };
                let parsed: T = serde_json::from_str(signals).map_err(
                    #[cfg_attr(not(feature = "tracing"), expect(unused_variables))]
                    |err| {
                        #[cfg(feature = "tracing")]
                        tracing::debug!(%err, "failed to parse JSON value");

                        error::ErrorBadRequest("Failed to parse JSON value")
                    },
                )?;
                Ok(ReadSignals(parsed))
            })
        } else {
            let json_fut = <web::Json<T> as FromRequest>::from_request(&req, payload);
            Box::pin(async move {
                let json = json_fut.await.map_err(
                    #[cfg_attr(not(feature = "tracing"), expect(unused_variables))]
                    |err| {
                        #[cfg(feature = "tracing")]
                        tracing::debug!(%err, "failed to parse JSON value from payload");

                        error::ErrorBadRequest("Failed to parse JSON value from payload")
                    },
                )?;
                Ok(ReadSignals(json.0))
            })
        }
    }
}

/// Datastar's headers
pub mod header {
    use {
        crate::consts::ElementPatchMode,
        actix_web::http::header::{HeaderName, HeaderValue},
    };

    /// A CSS selector for the target elements to patch
    pub const DATASTAR_SELECTOR: HeaderName = HeaderName::from_static("datastar-selector");

    /// How to patch the elements (See [`ElementPatchMode`]). Defaults to [`ElementPatchMode::Outer`].
    pub const DATASTAR_MODE: HeaderName = HeaderName::from_static("datastar-mode");

    /// Whether to use the [View Transition API](https://developer.mozilla.org/en-US/docs/Web/API/View_Transition_API) when patching elements.
    pub const DATASTAR_USE_VIEW_TRANSITION: HeaderName =
        HeaderName::from_static("datastar-use-view-transition");

    /// If set to true, only patch signals that don't already exist
    pub const DATASTAR_ONLY_IF_MISSING: HeaderName =
        HeaderName::from_static("datastar-only-if-missing");

    /// Sets the script element's attributes using a JSON encoded string.
    pub const DATASTAR_SCRIPT_ATTRIBUTES: HeaderName =
        HeaderName::from_static("datastar-script-attributes");

    impl From<ElementPatchMode> for HeaderValue {
        fn from(value: ElementPatchMode) -> Self {
            HeaderValue::from_static(value.as_str())
        }
    }
}
