use actix_sse::{Event, Sse};
use actix_web::{
    App, HttpServer, Responder,
    web::{self, Html, Path},
};
use asynk_strim::{Yielder, stream_fn};
use datastar::{
    actix_web::ReadSignals,
    prelude::{ElementPatchMode, PatchElements, PatchSignals},
};
use serde::{Deserialize, Serialize};
use {
    std::error::Error,
    std::time::Duration,
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
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/event/generate", web::post().to(generate))
            .route("/event/{status}", web::post().to(event))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await?;

    Ok(())
}

async fn index() -> Html {
    Html::new(include_str!("activity-feed.html"))
}

async fn generate(ReadSignals(signals): ReadSignals<Signals>) -> impl Responder {
    // Values we will update in a loop
    let mut total = signals.total;
    let mut done = signals.done;

    Sse::from_infallible_stream(stream_fn(move |mut yielder: Yielder<Event>| async move {
        // Signal event generation start
        let patch = PatchSignals::new(r#"{"generating": true}"#);
        let event = patch.into();
        yielder.yield_item(event).await;

        // Yield the events elements and signals to the stream
        for _ in 1..=signals.events {
            total += 1;
            done += 1;
            // Append a new entry to the activity feed
            let elements = event_entry(&Status::Done, total, "Auto");
            let patch = PatchElements::new(elements)
                .selector("#feed")
                .mode(ElementPatchMode::After);
            let event = patch.into();
            yielder.yield_item(event).await;

            // Update the event counts
            let patch = PatchSignals::new(format!(r#"{{"total": {total}, "done": {done}}}"#));
            let event = patch.into();
            yielder.yield_item(event).await;
            tokio::time::sleep(Duration::from_millis(signals.interval)).await;
        }

        // Signal event generation end
        let patch = PatchSignals::new(r#"{"generating": false}"#);
        let event = patch.into();
        yielder.yield_item(event).await;
    }))
}

/// Creates one event with a given status
async fn event(path: Path<Status>, ReadSignals(signals): ReadSignals<Signals>) -> impl Responder {
    // Create the event stream, since we're patching both an element and a signal.
    Sse::from_infallible_stream(stream_fn(move |mut yielder: Yielder<Event>| async move {
        // Signal the updated event counts
        let total = signals.total + 1;
        let status = path.into_inner();
        let signals = match status {
            Status::Done => format!(r#"{{"total": {total}, "done": {}}}"#, signals.done + 1),
            Status::Warn => format!(r#"{{"total": {total}, "warn": {}}}"#, signals.warn + 1),
            Status::Fail => format!(r#"{{"total": {total}, "fail": {}}}"#, signals.fail + 1),
            Status::Info => format!(r#"{{"total": {total}, "info": {}}}"#, signals.info + 1),
        };
        let patch = PatchSignals::new(signals);
        let signal = patch.into();
        yielder.yield_item(signal).await;

        // Patch an element and append it to the feed
        let elements = event_entry(&status, total, "Manual");
        let patch = PatchElements::new(elements)
            .selector("#feed")
            .mode(ElementPatchMode::After);
        let event = patch.into();
        yielder.yield_item(event).await;
    }))
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
