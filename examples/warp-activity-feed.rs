use {
    asynk_strim::{Yielder, stream_fn},
    core::{convert::Infallible, error::Error, str::FromStr, time::Duration},
    datastar::{
        prelude::{ElementPatchMode, PatchElements, PatchSignals},
        warp::{ReadSignals, read_signals},
    },
    serde::{Deserialize, Serialize},
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
    warp::{Filter, filters::sse::Event},
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
#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Done,
    Fail,
    Info,
    Warn,
}

impl FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "done" => Ok(Status::Done),
            "fail" => Ok(Status::Fail),
            "info" => Ok(Status::Info),
            "warn" => Ok(Status::Warn),
            _ => Err(format!("unknown status: {s}")),
        }
    }
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

    let index = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::html(include_str!("activity-feed.html")));

    let generate = warp::path!("event" / "generate")
        .and(warp::post())
        .and(read_signals::<Signals>())
        .map(|ReadSignals(signals): ReadSignals<Signals>| {
            // Values we will update in a loop
            let mut total = signals.total;
            let mut done = signals.done;

            let stream = stream_fn(
                move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
                    // Signal event generation start
                    let patch = PatchSignals::new(r#"{"generating": true}"#);
                    let sse_event = patch.write_as_warp_sse_event();
                    yielder.yield_item(Ok(sse_event)).await;

                    // Yield the events elements and signals to the stream
                    for _ in 1..=signals.events {
                        total += 1;
                        done += 1;
                        // Append a new entry to the activity feed
                        let elements = event_entry(&Status::Done, total, "Auto");
                        let patch = PatchElements::new(elements)
                            .selector("#feed")
                            .mode(ElementPatchMode::After);
                        let sse_event = patch.write_as_warp_sse_event();
                        yielder.yield_item(Ok(sse_event)).await;

                        // Update the event counts
                        let patch =
                            PatchSignals::new(format!(r#"{{"total": {total}, "done": {done}}}"#));
                        let sse_event = patch.write_as_warp_sse_event();
                        yielder.yield_item(Ok(sse_event)).await;
                        tokio::time::sleep(Duration::from_millis(signals.interval)).await;
                    }

                    // Signal event generation end
                    let patch = PatchSignals::new(r#"{"generating": false}"#);
                    let sse_event = patch.write_as_warp_sse_event();
                    yielder.yield_item(Ok(sse_event)).await;
                },
            );
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    let event = warp::path!("event" / Status)
        .and(warp::post())
        .and(read_signals::<Signals>())
        .map(
            |status: Status, ReadSignals(signals): ReadSignals<Signals>| {
                let stream = stream_fn(
                    move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
                        // Signal the updated event counts
                        let total = signals.total + 1;
                        let signals_json = match status {
                            Status::Done => {
                                format!(r#"{{"total": {total}, "done": {}}}"#, signals.done + 1)
                            }
                            Status::Warn => {
                                format!(r#"{{"total": {total}, "warn": {}}}"#, signals.warn + 1)
                            }
                            Status::Fail => {
                                format!(r#"{{"total": {total}, "fail": {}}}"#, signals.fail + 1)
                            }
                            Status::Info => {
                                format!(r#"{{"total": {total}, "info": {}}}"#, signals.info + 1)
                            }
                        };
                        let patch = PatchSignals::new(signals_json);
                        let sse_signal = patch.write_as_warp_sse_event();
                        yielder.yield_item(Ok(sse_signal)).await;

                        // Patch an element and append it to the feed
                        let elements = event_entry(&status, total, "Manual");
                        let patch = PatchElements::new(elements)
                            .selector("#feed")
                            .mode(ElementPatchMode::After);
                        let sse_event = patch.write_as_warp_sse_event();
                        yielder.yield_item(Ok(sse_event)).await;
                    },
                );
                warp::sse::reply(warp::sse::keep_alive().stream(stream))
            },
        );

    let routes = index.or(generate).or(event);

    tracing::debug!("listening on 127.0.0.1:3000");
    warp::serve(routes).run(([127, 0, 0, 1], 3000)).await;

    Ok(())
}

/// Returns an HTML string for the entry
fn event_entry(status: &Status, index: u64, source: &str) -> String {
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    let (color, indicator) = match status {
        Status::Done => ("green", "Done"),
        Status::Warn => ("yellow", "Warn"),
        Status::Fail => ("red", "Fail"),
        Status::Info => ("blue", "Info"),
    };
    format!(
        "<div id='event-{index}' class='text-{color}-500'>{timestamp} [ {indicator} ] {source} event {index}</div>"
    )
}
