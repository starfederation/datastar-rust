use {
    async_stream::stream,
    axum::{
        Router,
        extract::Path,
        response::{Html, IntoResponse, Sse},
        routing::{get, post},
    },
    chrono,
    core::{convert::Infallible, error::Error, time::Duration},
    datastar::{
        axum::ReadSignals,
        prelude::{ElementPatchMode, PatchElements, PatchSignals},
    },
    serde::{Deserialize, Serialize},
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
};

/// All `data-signals-*` defined in activity-feed.html
#[derive(Serialize, Deserialize)]
pub struct Signals {
    // Form inputs
    pub interval: u64,
    pub events: u64,
    // Activity flags
    pub generating: bool,
    // Output counters
    pub total: u64,
    pub done: u64,
    pub warn: u64,
    pub fail: u64,
    pub info: u64,
}

/// All valid event statuses.
// Normalizing variants to lowercase allows parsing routes from `/event/{status}`
// with a `Path<Status>` extractor.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Done,
    Fail,
    Info,
    Warn,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(index))
        .route("/event/generate", post(generate))
        .route("/event/{status}", post(event));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    tracing::debug!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Simple handler returning a static HTML page
async fn index() -> Html<&'static str> {
    Html(include_str!("activity-feed.html"))
}

/// Generates a number of "done" events with a specified interval.
async fn generate(ReadSignals(signals): ReadSignals<Signals>) -> impl IntoResponse {
    // Values we will update in a loop
    let mut total = signals.total;
    let mut done = signals.done;

    // Start the SSE stream
    Sse::new(stream! {
        // Signal event generation start
        let patch = PatchSignals::new(format!(r#"{{"generating": true}}"#));
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);

        // Yield the events elements and signals to the stream
        for _ in 1..=signals.events {
            total += 1;
            done += 1;
            // Append a new entry to the activity feed
            let elements = event_entry(&Status::Done, total, "Auto");
            let patch = PatchElements::new(elements).selector("#feed").mode(ElementPatchMode::After);
            let sse_event = patch.write_as_axum_sse_event();
            yield Ok::<_, Infallible>(sse_event);

            // Update the event counts
            let patch = PatchSignals::new(format!(r#"{{"total": {total}, "done": {done}}}"#));
            let sse_event = patch.write_as_axum_sse_event();
            yield Ok::<_, Infallible>(sse_event);
            tokio::time::sleep(Duration::from_millis(signals.interval)).await;
        }

        // Signal event generation end
        let patch = PatchSignals::new(format!(r#"{{"generating": false}}"#));
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);
    })
}

/// Creates one event with a given status
async fn event(
    Path(status): Path<Status>,
    ReadSignals(signals): ReadSignals<Signals>,
) -> impl IntoResponse {
    // Create the event stream, since we're patching both an element and a signal.
    Sse::new(stream! {
        // Signal the updated event counts
        let total = signals.total + 1;
        let signals = match status {
            Status::Done => format!(r#"{{"total": {total}, "done": {}}}"#, signals.done + 1),
            Status::Warn => format!(r#"{{"total": {total}, "warn": {}}}"#, signals.warn + 1),
            Status::Fail => format!(r#"{{"total": {total}, "fail": {}}}"#, signals.fail + 1),
            Status::Info => format!(r#"{{"total": {total}, "info": {}}}"#, signals.info + 1),
        };
        let patch = PatchSignals::new(signals);
        let sse_signal = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_signal);

        // Patch an element and append it to the feed
        let elements = event_entry(&status, total, "Manual");
        let patch = PatchElements::new(elements).selector("#feed").mode(ElementPatchMode::After);
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);
    })
}

/// Returns an HTML string for the entry
fn event_entry(status: &Status, index: u64, source: &str) -> String {
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    let (color, indicator) = match status {
        Status::Done => ("green", "✅ Done"),
        Status::Warn => ("yellow", "⚠️ Warn"),
        Status::Fail => ("red", "❌ Fail"),
        Status::Info => ("blue", "ℹ️ Info"),
    };
    format!(
        "<div id='event-{index}' class='text-{color}-500'>{timestamp} [ {indicator} ] {source} event {index}</div>"
    )
}
