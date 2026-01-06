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
    datastar: serde_json::Value,
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
        Method::GET => {
            // Parse ?datastar={json} from query string
            let params: DatastarParam = serde_urlencoded::from_str(&query).map_err(|err| {
                #[cfg(feature = "tracing")]
                tracing::debug!(%err, "failed to parse query string");

                warp::reject::custom(ReadSignalsError {
                    message: format!("Failed to parse query: {err}"),
                    status: StatusCode::BAD_REQUEST,
                })
            })?;

            let signals_str = params.datastar.as_str().ok_or_else(|| {
                warp::reject::custom(ReadSignalsError {
                    message: "datastar parameter must be a JSON string".into(),
                    status: StatusCode::BAD_REQUEST,
                })
            })?;

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
