use {
    async_stream::stream,
    axum::{
        Router,
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
        .route(
            "/event/info",
            post(move |signals| event(Status::Info, signals)),
        )
        .route(
            "/event/done",
            post(move |signals| event(Status::Done, signals)),
        )
        .route(
            "/event/warn",
            post(move |signals| event(Status::Warn, signals)),
        )
        .route(
            "/event/fail",
            post(move |signals| event(Status::Fail, signals)),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("activity-feed.html"))
}

#[derive(Serialize, Deserialize)]
pub struct Signals {
    pub interval: u64,
    pub events: u64,
    pub generating: bool,
    pub total: u64,
    pub done: u64,
    pub warn: u64,
    pub fail: u64,
    pub info: u64,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub enum Status {
    Info,
    Done,
    Warn,
    Fail,
}

async fn generate(ReadSignals(signals): ReadSignals<Signals>) -> impl IntoResponse {
    Sse::new(stream! {
        let mut total = signals.total;
        let mut done = signals.done;
        for _ in 1..=signals.events {
            total += 1;
            done += 1;
            let elements = event_entry(total, &Status::Done, "Auto");
            let patch = PatchElements::new(elements).selector("#feed").mode(ElementPatchMode::After);
            let sse_event = patch.write_as_axum_sse_event();
            yield Ok::<_, Infallible>(sse_event);
            let signals_generating = serde_json::to_string(&Signals{
                generating: true,
                total,
                done,
                ..signals
            }).unwrap();
            let patch = PatchSignals::new(signals_generating);
            let sse_event = patch.write_as_axum_sse_event();

            yield Ok::<_, Infallible>(sse_event);
            tokio::time::sleep(Duration::from_millis(signals.interval)).await;
        }

        let signals_done = serde_json::to_string(&Signals{
            generating: false,
            total,
            done,
            ..signals
        }).unwrap();
        let patch = PatchSignals::new(signals_done);
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);
    })
}

async fn event(status: Status, ReadSignals(signals): ReadSignals<Signals>) -> impl IntoResponse {
    Sse::new(stream! {
        let total = signals.total + 1;
        let mut done = signals.done;
        let mut warn = signals.warn;
        let mut fail = signals.fail;
        let mut info = signals.info;
        match status {
            Status::Done => done += 1,
            Status::Warn => warn += 1,
            Status::Fail => fail += 1,
            Status::Info => info += 1,
        }
        let elements = event_entry(total, &status, "Manual");
        let patch = PatchElements::new(elements).selector("#feed").mode(ElementPatchMode::After);
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);

        let signals = serde_json::to_string(&Signals {
            total,
            done,
            warn,
            fail,
            info,
            ..signals
        }).unwrap();
        let patch = PatchSignals::new(signals);
        let sse_signal = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_signal);
    })
}

fn event_entry(index: u64, status: &Status, prefix: &str) -> String {
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    match status {
        Status::Done => {
            format!(
                "<div id='event-{}' class='text-green-500'>{} [ ✅ Done ] {} event {}</div>",
                index, timestamp, prefix, index
            )
        }
        Status::Warn => {
            format!(
                "<div id='event-{}' class='text-yellow-500'>{} [ ⚠️ Warn ] {} event {}</div>",
                index, timestamp, prefix, index
            )
        }
        Status::Fail => {
            format!(
                "<div id='event-{}' class='text-red-500'>{} [ ❌ Fail ] {} event {}</div>",
                index, timestamp, prefix, index
            )
        }
        Status::Info => {
            format!(
                "<div id='event-{}' class='text-blue-500'>{} [ ℹ️ Info ] {} event {}</div>",
                index, timestamp, prefix, index
            )
        }
    }
}
