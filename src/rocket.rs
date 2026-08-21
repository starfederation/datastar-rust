//! Rocket integration for Datastar.

use {
    crate::{
        DatastarEvent,
        prelude::{ExecuteScript, PatchElements, PatchSignals},
    },
    rocket::{
        data::{Data, FromData},
        http::{Method, Status},
        outcome::Outcome,
        request::{FromRequest, Request},
        response::stream::Event,
        serde::{
            DeserializeOwned,
            json::{Json, serde_json},
        },
    },
    std::{fmt, fmt::Write},
};

/// Error returned when Datastar signals cannot be read from a query string.
#[derive(Debug)]
pub struct ReadSignalsError {
    message: String,
}

impl fmt::Display for ReadSignalsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ReadSignalsError {}

/// A request guard and data guard for reading Datastar signals.
///
/// GET and DELETE routes use this as a request guard and read the URL-encoded
/// `datastar` query parameter. A missing parameter is treated as JSON `null`,
/// allowing `ReadSignals<Option<T>>` to produce `None`.
///
/// POST, PUT, and PATCH routes use this as a data guard and read the JSON body:
///
/// ```
/// use datastar::rocket::ReadSignals;
/// use rocket::serde::Deserialize;
///
/// #[derive(Deserialize)]
/// #[serde(crate = "rocket::serde")]
/// struct Signals {
///     delay: u64,
/// }
///
/// #[rocket::get("/signals")]
/// fn get(signals: ReadSignals<Signals>) {
///     println!("delay: {}", signals.0.delay);
/// }
///
/// #[rocket::post("/signals", data = "<signals>")]
/// fn post(signals: ReadSignals<Signals>) {
///     println!("delay: {}", signals.0.delay);
/// }
/// ```
#[derive(Debug)]
pub struct ReadSignals<T>(pub T);

#[rocket::async_trait]
impl<'r, T> FromRequest<'r> for ReadSignals<T>
where
    T: DeserializeOwned,
{
    type Error = ReadSignalsError;

    async fn from_request(request: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        if !matches!(request.method(), Method::Get | Method::Delete) {
            return Outcome::Error((
                Status::BadRequest,
                ReadSignalsError {
                    message: format!(
                        "{} signals must be read with a Rocket data guard",
                        request.method()
                    ),
                },
            ));
        }

        let signals = request
            .uri()
            .query()
            .and_then(|query| {
                query
                    .segments()
                    .find_map(|(key, value)| (key == "datastar").then_some(value))
            })
            .unwrap_or("null");

        match serde_json::from_str(signals) {
            Ok(signals) => Outcome::Success(Self(signals)),
            Err(error) => Outcome::Error((
                Status::BadRequest,
                ReadSignalsError {
                    message: format!("failed to parse Datastar signals from query: {error}"),
                },
            )),
        }
    }
}

#[rocket::async_trait]
impl<'r, T> FromData<'r> for ReadSignals<T>
where
    T: DeserializeOwned,
{
    type Error = rocket::serde::json::Error<'r>;

    async fn from_data(
        request: &'r Request<'_>,
        data: Data<'r>,
    ) -> rocket::data::Outcome<'r, Self> {
        match <Json<T> as FromData<'r>>::from_data(request, data).await {
            Outcome::Success(Json(signals)) => Outcome::Success(Self(signals)),
            Outcome::Error(error) => Outcome::Error(error),
            Outcome::Forward(forward) => Outcome::Forward(forward),
        }
    }
}

impl PatchElements {
    /// Write this [`PatchElements`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_rocket_sse_event()
    }
}

impl From<PatchElements> for Event {
    fn from(value: PatchElements) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&PatchElements> for Event {
    fn from(value: &PatchElements) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl PatchSignals {
    /// Write this [`PatchSignals`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_rocket_sse_event()
    }
}

impl From<PatchSignals> for Event {
    fn from(value: PatchSignals) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&PatchSignals> for Event {
    fn from(value: &PatchSignals) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl ExecuteScript {
    /// Write this [`ExecuteScript`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_rocket_sse_event()
    }
}

impl From<ExecuteScript> for Event {
    fn from(value: ExecuteScript) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&ExecuteScript> for Event {
    fn from(value: &ExecuteScript) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl DatastarEvent {
    /// Turn this [`DatastarEvent`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        let mut data = String::with_capacity(
            (self.data.iter().map(|s| s.len()).sum::<usize>() + self.data.len()).saturating_sub(1),
        );

        let mut sep = "";
        for line in self.data.iter() {
            // Assumption: std::fmt::write does not fail ever for [`String`].
            let _ = write!(&mut data, "{sep}{line}");
            sep = "\n";
        }

        let mut event = Event::data(data).event(self.event.as_str().to_owned());

        if self.retry.as_millis() != (crate::consts::DEFAULT_SSE_RETRY_DURATION as u128) {
            event = event.with_retry(self.retry);
        }

        match self.id.as_deref() {
            Some(id) => event.id(id.to_owned()),
            None => event,
        }
    }
}

impl From<DatastarEvent> for Event {
    fn from(value: DatastarEvent) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&DatastarEvent> for Event {
    fn from(value: &DatastarEvent) -> Self {
        value.write_as_rocket_sse_event()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::consts::{self, EventType},
        core::time::Duration,
        rocket::{http::ContentType, local::blocking::Client, routes, serde::Deserialize},
    };

    #[derive(Debug, Deserialize)]
    #[serde(crate = "rocket::serde")]
    struct TestSignals {
        delay: u64,
    }

    #[rocket::get("/signals")]
    fn get_signals(signals: ReadSignals<TestSignals>) -> String {
        signals.0.delay.to_string()
    }

    #[rocket::delete("/signals")]
    fn delete_signals(signals: ReadSignals<TestSignals>) -> String {
        signals.0.delay.to_string()
    }

    #[rocket::post("/signals", data = "<signals>")]
    fn post_signals(signals: ReadSignals<TestSignals>) -> String {
        signals.0.delay.to_string()
    }

    #[rocket::put("/signals", data = "<signals>")]
    fn put_signals(signals: ReadSignals<TestSignals>) -> String {
        signals.0.delay.to_string()
    }

    #[rocket::patch("/signals", data = "<signals>")]
    fn patch_signals(signals: ReadSignals<TestSignals>) -> String {
        signals.0.delay.to_string()
    }

    #[rocket::get("/optional")]
    fn optional_signals(signals: ReadSignals<Option<TestSignals>>) -> &'static str {
        if signals.0.is_some() { "some" } else { "none" }
    }

    fn client() -> Client {
        Client::tracked(rocket::build().mount(
            "/",
            routes![
                get_signals,
                delete_signals,
                post_signals,
                put_signals,
                patch_signals,
                optional_signals,
            ],
        ))
        .expect("valid Rocket instance")
    }

    fn assert_event(event: Event, expected: &Event) {
        assert_eq!(&event, expected);
    }

    #[test]
    fn reads_query_signals_for_get_and_delete() {
        let client = client();
        let uri = "/signals?datastar=%7B%22delay%22%3A250%7D";

        let get = client.get(uri).dispatch();
        assert_eq!(get.status(), Status::Ok);
        assert_eq!(get.into_string().as_deref(), Some("250"));

        let delete = client.delete(uri).dispatch();
        assert_eq!(delete.status(), Status::Ok);
        assert_eq!(delete.into_string().as_deref(), Some("250"));
    }

    #[test]
    fn reads_body_signals_for_post_put_and_patch() {
        let client = client();
        let body = r#"{"delay":250}"#;

        let post = client
            .post("/signals")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(post.status(), Status::Ok);
        assert_eq!(post.into_string().as_deref(), Some("250"));

        let put = client
            .put("/signals")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(put.status(), Status::Ok);
        assert_eq!(put.into_string().as_deref(), Some("250"));

        let patch = client
            .patch("/signals")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(patch.status(), Status::Ok);
        assert_eq!(patch.into_string().as_deref(), Some("250"));
    }

    #[test]
    fn handles_missing_and_invalid_query_signals() {
        let client = client();

        let optional = client.get("/optional").dispatch();
        assert_eq!(optional.status(), Status::Ok);
        assert_eq!(optional.into_string().as_deref(), Some("none"));

        let missing = client.get("/signals").dispatch();
        assert_eq!(missing.status(), Status::BadRequest);

        let invalid = client.get("/signals?datastar=invalid").dispatch();
        assert_eq!(invalid.status(), Status::BadRequest);
    }

    #[test]
    fn writes_patch_elements_and_conversions() {
        let patch = PatchElements::new("<div>hello</div>")
            .id("elements-1")
            .retry(Duration::from_millis(2_500));
        let expected = Event::data("elements <div>hello</div>")
            .event("datastar-patch-elements")
            .with_retry(Duration::from_millis(2_500))
            .id("elements-1");

        assert_event(patch.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&patch), &expected);
        assert_event(Event::from(patch), &expected);
    }

    #[test]
    fn writes_patch_signals_and_conversions() {
        let patch = PatchSignals::new("{count: 1}").only_if_missing(true);
        let expected =
            Event::data("onlyIfMissing true\nsignals {count: 1}").event("datastar-patch-signals");

        assert_event(patch.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&patch), &expected);
        assert_event(Event::from(patch), &expected);
    }

    #[test]
    fn writes_execute_script_and_conversions() {
        let script = ExecuteScript::new("console.log('hello')");
        let expected = Event::data(concat!(
            "selector body\n",
            "mode append\n",
            "elements <script data-effect=\"el.remove()\">",
            "console.log('hello')</script>",
        ))
        .event("datastar-patch-elements");

        assert_event(script.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&script), &expected);
        assert_event(Event::from(script), &expected);
    }

    #[test]
    fn writes_generic_events_and_conversions() {
        let event = DatastarEvent {
            event: EventType::PatchSignals,
            id: Some("signals-1".into()),
            retry: Duration::from_millis(2_500),
            data: vec!["signals {count: 1}".into()],
        };
        let expected = Event::data("signals {count: 1}")
            .event("datastar-patch-signals")
            .with_retry(Duration::from_millis(2_500))
            .id("signals-1");

        assert_event(event.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&event), &expected);
        assert_event(Event::from(event), &expected);
    }

    #[test]
    fn writes_empty_generic_data() {
        let event = DatastarEvent {
            event: EventType::PatchElements,
            id: None,
            retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
            data: Vec::new(),
        };
        let expected = Event::data("").event("datastar-patch-elements");

        assert_event(event.write_as_rocket_sse_event(), &expected);
    }
}
