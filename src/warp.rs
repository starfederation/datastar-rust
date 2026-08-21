//! Warp integration for Datastar.

use {
    crate::{
        consts::{self, DATASTAR_REQ_HEADER_STR},
        prelude::{DatastarEvent, ExecuteScript, PatchElements, PatchSignals},
    },
    bytes::Bytes,
    serde::{Deserialize, de::DeserializeOwned},
    std::{convert::Infallible, fmt::Write},
    warp::{
        Filter, Rejection, Reply,
        filters::sse::Event,
        http::{Method, StatusCode},
    },
};

impl PatchElements {
    /// Write this [`PatchElements`] into a Warp SSE [`Event`].
    pub fn write_as_warp_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_warp_sse_event()
    }
}

impl From<PatchElements> for Event {
    fn from(value: PatchElements) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl From<&PatchElements> for Event {
    fn from(value: &PatchElements) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl PatchSignals {
    /// Write this [`PatchSignals`] into a Warp SSE [`Event`].
    pub fn write_as_warp_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_warp_sse_event()
    }
}

impl From<PatchSignals> for Event {
    fn from(value: PatchSignals) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl From<&PatchSignals> for Event {
    fn from(value: &PatchSignals) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl ExecuteScript {
    /// Write this [`ExecuteScript`] into a Warp SSE [`Event`].
    pub fn write_as_warp_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_warp_sse_event()
    }
}

impl From<ExecuteScript> for Event {
    fn from(value: ExecuteScript) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl From<&ExecuteScript> for Event {
    fn from(value: &ExecuteScript) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl DatastarEvent {
    /// Turn this [`DatastarEvent`] into a Warp SSE [`Event`].
    pub fn write_as_warp_sse_event(&self) -> Event {
        let mut event = Event::default().event(self.event.as_str());

        if self.retry.as_millis() != (consts::DEFAULT_SSE_RETRY_DURATION as u128) {
            event = event.retry(self.retry);
        }

        event = match self.id.as_deref() {
            Some(id) => event.id(id),
            None => event,
        };

        let mut data = String::with_capacity(
            (self.data.iter().map(|s| s.len()).sum::<usize>() + self.data.len()).saturating_sub(1),
        );

        let mut sep = "";
        for line in self.data.iter() {
            // Assumption: std::fmt::write does not fail ever for [`String`].
            let _ = write!(&mut data, "{sep}{line}");
            sep = "\n";
        }

        event.data(data)
    }
}

impl From<DatastarEvent> for Event {
    fn from(value: DatastarEvent) -> Self {
        value.write_as_warp_sse_event()
    }
}

impl From<&DatastarEvent> for Event {
    fn from(value: &DatastarEvent) -> Self {
        value.write_as_warp_sse_event()
    }
}

#[derive(Deserialize)]
struct DatastarParam {
    datastar: Option<serde_json::Value>,
}

/// Error type for [`ReadSignals`] extraction failures.
#[derive(Debug)]
pub struct ReadSignalsError {
    message: String,
    status: StatusCode,
}

impl warp::reject::Reject for ReadSignalsError {}

/// [`ReadSignals`] is a wrapper type for extracted Datastar signals.
///
/// # Examples
///
/// ```
/// use datastar::warp::{read_signals, ReadSignals};
/// use serde::Deserialize;
/// use warp::Filter;
///
/// #[derive(Deserialize)]
/// struct Signals {
///     foo: String,
///     bar: i32,
/// }
///
/// let route = warp::path("hello")
///     .and(read_signals::<Signals>())
///     .map(|signals: ReadSignals<Signals>| {
///         format!("foo: {}, bar: {}", signals.0.foo, signals.0.bar)
///     });
/// ```
#[derive(Debug)]
pub struct ReadSignals<T>(pub T);

/// Creates a Warp Filter that extracts Datastar signals from the request.
///
/// For GET requests, signals are extracted from the `datastar` query parameter.
/// For POST/PUT/PATCH requests, signals are extracted from the JSON body.
///
/// # Examples
///
/// ```
/// use datastar::warp::{read_signals, ReadSignals};
/// use serde::Deserialize;
/// use warp::Filter;
///
/// #[derive(Deserialize)]
/// struct Signals {
///     delay: u64,
/// }
///
/// let route = warp::path("hello")
///     .and(warp::get())
///     .and(read_signals::<Signals>())
///     .map(|ReadSignals(signals): ReadSignals<Signals>| {
///         format!("delay: {}", signals.delay)
///     });
/// ```
pub fn read_signals<T>() -> impl Filter<Extract = (ReadSignals<T>,), Error = Rejection> + Clone
where
    T: DeserializeOwned + Send,
{
    warp::method()
        .and(warp::query::raw().or(warp::any().map(String::new)).unify())
        .and(warp::body::bytes().or(warp::any().map(Bytes::new)).unify())
        .and_then(extract_signals::<T>)
}

async fn extract_signals<T>(
    method: Method,
    query: String,
    body: Bytes,
) -> Result<ReadSignals<T>, Rejection>
where
    T: DeserializeOwned,
{
    match method {
        Method::GET | Method::DELETE => {
            // Parse ?datastar={json} from query string
            let params: DatastarParam = serde_urlencoded::from_str(&query).map_err(|err| {
                #[cfg(feature = "tracing")]
                tracing::debug!(%err, "failed to parse query string");

                warp::reject::custom(ReadSignalsError {
                    message: format!("Failed to parse query: {err}"),
                    status: StatusCode::BAD_REQUEST,
                })
            })?;

            let signals_str = match params.datastar.as_ref() {
                Some(value) => value.as_str().ok_or_else(|| {
                    warp::reject::custom(ReadSignalsError {
                        message: "datastar parameter must be a JSON string".into(),
                        status: StatusCode::BAD_REQUEST,
                    })
                })?,
                None => "null",
            };

            let signals: T = serde_json::from_str(signals_str).map_err(|err| {
                #[cfg(feature = "tracing")]
                tracing::debug!(%err, "failed to parse JSON value from query");

                let _ = &err; // silence unused warning when tracing is disabled

                warp::reject::custom(ReadSignalsError {
                    message: format!("Failed to parse JSON: {err}"),
                    status: StatusCode::BAD_REQUEST,
                })
            })?;

            Ok(ReadSignals(signals))
        }
        _ => {
            // POST/PUT/PATCH: parse body as JSON
            let signals: T = serde_json::from_slice(&body).map_err(|err| {
                #[cfg(feature = "tracing")]
                tracing::debug!(%err, "failed to parse JSON value from body");

                let _ = &err; // silence unused warning when tracing is disabled

                warp::reject::custom(ReadSignalsError {
                    message: format!("Failed to parse JSON body: {err}"),
                    status: StatusCode::BAD_REQUEST,
                })
            })?;

            Ok(ReadSignals(signals))
        }
    }
}

/// Creates a Filter that checks for the datastar-request header.
/// Returns `true` if the header is present, `false` otherwise.
pub fn is_datastar_request() -> impl Filter<Extract = (bool,), Error = Rejection> + Clone {
    warp::header::optional::<String>(DATASTAR_REQ_HEADER_STR)
        .map(|header: Option<String>| header.is_some())
}

/// Creates a Filter that optionally extracts Datastar signals from the request.
///
/// Returns `Some(ReadSignals<T>)` if signals are present and parseable,
/// `None` if the `datastar-request` header is not present.
///
/// # Examples
///
/// ```
/// use datastar::warp::{read_signals_optional, ReadSignals};
/// use serde::Deserialize;
/// use warp::Filter;
///
/// #[derive(Deserialize)]
/// struct Signals {
///     delay: u64,
/// }
///
/// let route = warp::path("hello")
///     .and(read_signals_optional::<Signals>())
///     .map(|signals: Option<ReadSignals<Signals>>| {
///         match signals {
///             Some(ReadSignals(s)) => format!("delay: {}", s.delay),
///             None => "no signals".to_string(),
///         }
///     });
/// ```
pub fn read_signals_optional<T>()
-> impl Filter<Extract = (Option<ReadSignals<T>>,), Error = Rejection> + Clone
where
    T: DeserializeOwned + Send,
{
    warp::header::optional::<String>(DATASTAR_REQ_HEADER_STR)
        .and(
            read_signals::<T>()
                .map(Some)
                .or(warp::any().map(|| None::<ReadSignals<T>>))
                .unify(),
        )
        .map(
            |is_datastar: Option<String>, signals: Option<ReadSignals<T>>| {
                if is_datastar.is_some() { signals } else { None }
            },
        )
}

/// Rejection handler for [`ReadSignals`] errors.
///
/// Use this with `warp::Filter::recover` to convert rejections into proper HTTP responses.
///
/// # Examples
///
/// ```
/// use datastar::warp::{read_signals, handle_rejection, ReadSignals};
/// use serde::Deserialize;
/// use warp::Filter;
///
/// #[derive(Deserialize)]
/// struct Signals {
///     delay: u64,
/// }
///
/// let route = warp::path("hello")
///     .and(read_signals::<Signals>())
///     .map(|ReadSignals(signals): ReadSignals<Signals>| {
///         format!("delay: {}", signals.delay)
///     })
///     .recover(handle_rejection);
/// ```
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    if let Some(e) = err.find::<ReadSignalsError>() {
        Ok(warp::reply::with_status(e.message.clone(), e.status))
    } else {
        Ok(warp::reply::with_status(
            "Internal Server Error".to_owned(),
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::consts::ElementPatchMode, core::time::Duration, serde::Deserialize};

    fn assert_event(event: Event, expected: &str) {
        assert_eq!(event.to_string(), expected);
    }

    #[test]
    fn writes_patch_elements_and_conversions() {
        let patch = PatchElements::new("<div>one</div>\n<div>two</div>")
            .id("elements-1")
            .retry(Duration::from_millis(2_500))
            .selector("#main")
            .mode(ElementPatchMode::Append);
        let expected = concat!(
            "event:datastar-patch-elements\n",
            "data:selector #main\n",
            "data:mode append\n",
            "data:elements <div>one</div>\n",
            "data:elements <div>two</div>\n",
            "id:elements-1\n",
            "retry:2500\n\n",
        );

        assert_event(patch.write_as_warp_sse_event(), expected);
        assert_event(Event::from(&patch), expected);
        assert_event(Event::from(patch), expected);
    }

    #[test]
    fn writes_patch_signals_and_conversions() {
        let patch = PatchSignals::new("{count: 1}").only_if_missing(true);
        let expected = concat!(
            "event:datastar-patch-signals\n",
            "data:onlyIfMissing true\n",
            "data:signals {count: 1}\n\n",
        );

        assert_event(patch.write_as_warp_sse_event(), expected);
        assert_event(Event::from(&patch), expected);
        assert_event(Event::from(patch), expected);
    }

    #[test]
    fn writes_execute_script_and_conversions() {
        let script = ExecuteScript::new("console.log('hello')");
        let expected = concat!(
            "event:datastar-patch-elements\n",
            "data:selector body\n",
            "data:mode append\n",
            "data:elements <script data-effect=\"el.remove()\">",
            "console.log('hello')</script>\n\n",
        );

        assert_event(script.write_as_warp_sse_event(), expected);
        assert_event(Event::from(&script), expected);
        assert_event(Event::from(script), expected);
    }

    #[test]
    fn writes_generic_events_and_conversions() {
        let event = PatchSignals::new("{count: 1}")
            .id("signals-1")
            .retry(Duration::from_millis(2_500))
            .into_datastar_event();
        let expected = concat!(
            "event:datastar-patch-signals\n",
            "data:signals {count: 1}\n",
            "id:signals-1\n",
            "retry:2500\n\n",
        );

        assert_event(event.write_as_warp_sse_event(), expected);
        assert_event(Event::from(&event), expected);
        assert_event(Event::from(event), expected);
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestSignals {
        count: u64,
    }

    #[tokio::test]
    async fn extracts_get_and_body_signals() {
        let get = warp::test::request()
            .method("GET")
            .path("/?datastar=%7B%22count%22%3A7%7D")
            .filter(&read_signals::<TestSignals>())
            .await
            .unwrap();
        assert_eq!(get.0, TestSignals { count: 7 });

        let delete = warp::test::request()
            .method("DELETE")
            .path("/?datastar=%7B%22count%22%3A8%7D")
            .filter(&read_signals::<TestSignals>())
            .await
            .unwrap();
        assert_eq!(delete.0, TestSignals { count: 8 });

        let post = warp::test::request()
            .method("POST")
            .body(r#"{"count":9}"#)
            .filter(&read_signals::<TestSignals>())
            .await
            .unwrap();
        assert_eq!(post.0, TestSignals { count: 9 });
    }

    #[tokio::test]
    async fn handles_optional_signals_and_request_header() {
        let present = warp::test::request()
            .method("GET")
            .path("/?datastar=%7B%22count%22%3A7%7D")
            .header(DATASTAR_REQ_HEADER_STR, "true")
            .filter(&read_signals_optional::<TestSignals>())
            .await
            .unwrap();
        assert_eq!(present.unwrap().0, TestSignals { count: 7 });

        let missing = warp::test::request()
            .filter(&read_signals_optional::<TestSignals>())
            .await
            .unwrap();
        assert!(missing.is_none());

        let present = warp::test::request()
            .header(DATASTAR_REQ_HEADER_STR, "true")
            .filter(&is_datastar_request())
            .await
            .unwrap();
        assert!(present);

        let missing = warp::test::request()
            .filter(&is_datastar_request())
            .await
            .unwrap();
        assert!(!missing);
    }

    #[tokio::test]
    async fn handles_missing_get_signals() {
        let optional = warp::test::request()
            .method("GET")
            .filter(&read_signals::<Option<TestSignals>>())
            .await
            .unwrap();
        assert_eq!(optional.0, None);

        let rejection = warp::test::request()
            .method("GET")
            .filter(&read_signals::<TestSignals>())
            .await
            .unwrap_err();
        let response = handle_rejection(rejection).await.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn maps_signal_rejections_to_responses() {
        let rejection = warp::test::request()
            .method("GET")
            .path("/?datastar=not-json")
            .filter(&read_signals::<TestSignals>())
            .await
            .unwrap_err();
        let response = handle_rejection(rejection).await.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = handle_rejection(warp::reject::not_found())
            .await
            .unwrap()
            .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
