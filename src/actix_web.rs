//! Actix-Web integration for Datastar

use {
    crate::prelude::{DatastarEvent, ExecuteScript, PatchElements, PatchSignals},
    actix_sse::{Data, Event},
    actix_web::{FromRequest, HttpRequest, dev::Payload, error, http::Method, web},
    serde::{Deserialize, de::DeserializeOwned},
    std::{fmt::Write, future::Future, pin::Pin},
};

impl PatchElements {
    /// Write this [`PatchElements`] into a Actix Web SSE [`Event`].
    pub fn write_as_actix_web_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_actix_web_sse_event()
    }
}

impl From<PatchElements> for Event {
    fn from(value: PatchElements) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl From<&PatchElements> for Event {
    fn from(value: &PatchElements) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl PatchSignals {
    /// Write this [`PatchSignals`] into a Actix Web SSE [`Event`].
    pub fn write_as_actix_web_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_actix_web_sse_event()
    }
}

impl From<PatchSignals> for Event {
    fn from(value: PatchSignals) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl From<&PatchSignals> for Event {
    fn from(value: &PatchSignals) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl ExecuteScript {
    /// Write this [`ExecuteScript`] into a Actix SSE [`Event`].
    pub fn write_as_actix_web_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_actix_web_sse_event()
    }
}

impl From<ExecuteScript> for Event {
    fn from(value: ExecuteScript) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl From<&ExecuteScript> for Event {
    fn from(value: &ExecuteScript) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl DatastarEvent {
    /// Turn this [`DatastarEvent`] into an Actix Web SSE [`Event`].
    pub fn write_as_actix_web_sse_event(&self) -> Event {
        let mut data = String::with_capacity(
            (self.data.iter().map(|s| s.len()).sum::<usize>() + self.data.len()).saturating_sub(1),
        );

        let mut sep = "";
        for line in self.data.iter() {
            // Assumption: std::fmt::write does not fail ever for [`String`].
            let _ = write!(&mut data, "{sep}{line}");
            sep = "\n";
        }

        let data = Data::new(data).event(self.event.as_str());

        let data = match self.id.as_deref() {
            Some(id) => data.id(id),
            None => data,
        };

        let data = if self.retry.as_millis() != crate::consts::DEFAULT_SSE_RETRY_DURATION as u128 {
            data.retry(self.retry)
        } else {
            data
        };

        Event::Data(data)
    }
}

impl From<DatastarEvent> for Event {
    fn from(value: DatastarEvent) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

impl From<&DatastarEvent> for Event {
    fn from(value: &DatastarEvent) -> Self {
        value.write_as_actix_web_sse_event()
    }
}

#[derive(Deserialize)]
struct DatastarParam {
    datastar: Option<serde_json::Value>,
}

/// [`ReadSignals`] is a request extractor that reads datastar signals from the request.
///
/// GET and DELETE requests read signals from the `datastar` query parameter.
/// A missing parameter is treated as JSON `null`, allowing
/// `ReadSignals<Option<T>>` to produce `None`. Other methods read signals from
/// the JSON request body, including Datastar 1.0.4's `QUERY` method.
/// Route `QUERY` requests with `web::method(Method::from_bytes(b"QUERY")?)`
/// or `web::route()`; Actix-Web's method guards do not include custom methods.
///
/// # Examples
///
/// ```
/// use datastar::actix_web::ReadSignals;
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
        crate::consts::{ElementPatchMode, Namespace},
        actix_web::http::header::{HeaderName, HeaderValue},
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
        crate::consts::{DATASTAR_REQ_HEADER_STR, ElementPatchMode, Namespace},
        actix_sse::Sse,
        actix_web::{
            FromRequest, Responder,
            body::to_bytes,
            http::{Method, StatusCode, header},
            test::TestRequest,
            web::Bytes,
        },
        core::time::Duration,
        tokio_stream::iter,
    };

    async fn render(event: Event) -> String {
        let req = TestRequest::default().to_http_request();
        let response = Sse::from_infallible_stream(iter([event])).respond_to(&req);
        let body = to_bytes(response.into_body()).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[actix_web::test]
    async fn writes_patch_elements_directly() {
        let event = PatchElements::new("<circle id=\"dot\" />\n<path />")
            .id("elements-1")
            .selector("#vis")
            .mode(ElementPatchMode::Append)
            .use_view_transition(true)
            .view_transition_selector("#main")
            .namespace(Namespace::Svg);

        assert_eq!(
            render(event.write_as_actix_web_sse_event()).await,
            concat!(
                "id: elements-1\n",
                "event: datastar-patch-elements\n",
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

    #[actix_web::test]
    async fn writes_custom_retry_duration() {
        let event = PatchSignals::new("{count: 1}").retry(Duration::from_millis(2_500));

        assert_eq!(
            render(event.write_as_actix_web_sse_event()).await,
            concat!(
                "event: datastar-patch-signals\n",
                "retry: 2500\n",
                "data: signals {count: 1}\n\n",
            )
        );
    }

    #[actix_web::test]
    async fn writes_patch_signals_directly() {
        let event = PatchSignals::new("{foo: 1,\nbar: 2}").only_if_missing(true);

        assert_eq!(
            render(event.write_as_actix_web_sse_event()).await,
            concat!(
                "event: datastar-patch-signals\n",
                "data: onlyIfMissing true\n",
                "data: signals {foo: 1,\n",
                "data: signals bar: 2}\n\n",
            )
        );
    }

    #[actix_web::test]
    async fn writes_execute_script_directly() {
        let event = ExecuteScript::new("console.log('one')\nconsole.log('two')")
            .auto_remove(false)
            .attributes([r#"type="module""#, "defer"]);

        assert_eq!(
            render(event.write_as_actix_web_sse_event()).await,
            concat!(
                "event: datastar-patch-elements\n",
                "data: selector body\n",
                "data: mode append\n",
                "data: elements <script type=\"module\" defer>console.log('one')\n",
                "data: elements console.log('two')</script>\n\n",
            )
        );
    }

    #[actix_web::test]
    async fn writes_generic_events_without_joining_data() {
        let event = PatchElements::new("<div>one</div>\n<div>two</div>").as_datastar_event();

        assert_eq!(
            render(event.write_as_actix_web_sse_event()).await,
            concat!(
                "event: datastar-patch-elements\n",
                "data: elements <div>one</div>\n",
                "data: elements <div>two</div>\n\n",
            )
        );
    }

    #[actix_web::test]
    async fn conversions_match_direct_writers() {
        let elements = PatchElements::new("<div>hello</div>");
        let expected = render(elements.write_as_actix_web_sse_event()).await;
        assert_eq!(render(Event::from(&elements)).await, expected);
        assert_eq!(render(Event::from(elements)).await, expected);

        let signals = PatchSignals::new("{count: 1}");
        let expected = render(signals.write_as_actix_web_sse_event()).await;
        assert_eq!(render(Event::from(&signals)).await, expected);
        assert_eq!(render(Event::from(signals)).await, expected);

        let script = ExecuteScript::new("console.log('hello')");
        let expected = render(script.write_as_actix_web_sse_event()).await;
        assert_eq!(render(Event::from(&script)).await, expected);
        assert_eq!(render(Event::from(script)).await, expected);

        let generic = PatchElements::new("<div>hello</div>").into_datastar_event();
        let expected = render(generic.write_as_actix_web_sse_event()).await;
        assert_eq!(render(Event::from(&generic)).await, expected);
        assert_eq!(render(Event::from(generic)).await, expected);
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestSignals {
        count: u64,
    }

    #[actix_web::test]
    async fn extracts_get_signals_from_query() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::GET)
            .uri("/?datastar=%7B%22count%22%3A7%7D")
            .insert_header((DATASTAR_REQ_HEADER_STR, "true"))
            .to_http_parts();

        let extracted = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
            .await
            .unwrap();

        assert_eq!(extracted.0, TestSignals { count: 7 });
    }

    #[actix_web::test]
    async fn extracts_delete_signals_from_query() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::DELETE)
            .uri("/?datastar=%7B%22count%22%3A8%7D")
            .to_http_parts();

        let extracted = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
            .await
            .unwrap();

        assert_eq!(extracted.0, TestSignals { count: 8 });
    }

    #[actix_web::test]
    async fn extracts_missing_get_signals_as_none() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::GET)
            .uri("/")
            .to_http_parts();

        let extracted = ReadSignals::<Option<TestSignals>>::from_request(&req, &mut payload)
            .await
            .unwrap();

        assert_eq!(extracted.0, None);
    }

    #[actix_web::test]
    async fn rejects_missing_required_get_signals() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::GET)
            .uri("/")
            .to_http_parts();

        let err = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(
            err.as_response_error().status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[actix_web::test]
    async fn extracts_post_signals() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::POST)
            .insert_header((header::CONTENT_TYPE, "application/json"))
            .set_payload(Bytes::from_static(br#"{"count":9}"#))
            .to_http_parts();

        let extracted = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
            .await
            .unwrap();

        assert_eq!(extracted.0, TestSignals { count: 9 });
    }

    #[actix_web::test]
    async fn extracts_query_method_signals_from_body() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::from_bytes(b"QUERY").unwrap())
            .uri("/?datastar=%7B%22count%22%3A1%7D")
            .insert_header((header::CONTENT_TYPE, "application/json"))
            .insert_header((DATASTAR_REQ_HEADER_STR, "true"))
            .set_payload(Bytes::from_static(br#"{"count":9}"#))
            .to_http_parts();

        let extracted = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
            .await
            .unwrap();

        assert_eq!(extracted.0, TestSignals { count: 9 });
    }

    #[actix_web::test]
    async fn rejects_invalid_query_method_bodies() {
        for body in ["", "not-json", r#"{"count":"invalid"}"#] {
            let (req, mut payload) = TestRequest::default()
                .method(Method::from_bytes(b"QUERY").unwrap())
                .uri("/?datastar=%7B%22count%22%3A1%7D")
                .insert_header((header::CONTENT_TYPE, "application/json"))
                .set_payload(Bytes::copy_from_slice(body.as_bytes()))
                .to_http_parts();

            let err = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
                .await
                .unwrap_err();

            assert_eq!(
                err.as_response_error().status_code(),
                StatusCode::BAD_REQUEST
            );
        }
    }

    #[actix_web::test]
    async fn rejects_invalid_get_signals() {
        let (req, mut payload) = TestRequest::default()
            .method(Method::GET)
            .uri("/?datastar=not-json")
            .to_http_parts();

        let err = ReadSignals::<TestSignals>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(
            err.as_response_error().status_code(),
            StatusCode::BAD_REQUEST
        );
    }
}
