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
    std::sync::Arc,
    tokio::sync::RwLock,
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
};

pub struct State {
    pub feed: Vec<String>,
    pub count: Count,
}

pub struct Count {
    pub all: u32,
    pub done: u32,
    pub warn: u32,
    pub fail: u32,
    pub info: u32,
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

    let state = Arc::new(RwLock::new(State {
        feed: Vec::new(),
        count: Count {
            all: 0,
            done: 0,
            warn: 0,
            fail: 0,
            info: 0,
        },
    }));

    let state_generate = state.clone();
    let state_info = state.clone();
    let state_done = state.clone();
    let state_warn = state.clone();
    let state_fail = state.clone();

    let event = { move |level, state| async move { event(level, state).await } };
    let generate = { move |signals| async move { generate(signals, state_generate).await } };

    let app = Router::new()
        .route("/", get(index))
        .route("/event/generate", post(generate))
        .route("/event/info", post(move || event(Status::Info, state_info)))
        .route("/event/done", post(move || event(Status::Done, state_done)))
        .route("/event/warn", post(move || event(Status::Warn, state_warn)))
        .route("/event/fail", post(move || event(Status::Fail, state_fail)));

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
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub enum Status {
    Info,
    Done,
    Warn,
    Fail,
}

async fn generate(
    ReadSignals(signals): ReadSignals<Signals>,
    state: Arc<RwLock<State>>,
) -> impl IntoResponse {
    Sse::new(stream! {
        let elements = r#"<div id="feed"></div>"#.to_string();
        let patch = PatchElements::new(elements);
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);

        let signals_generating = serde_json::to_string(&Signals{
            generating: true,
            interval: signals.interval,
            events: signals.events,
        }).unwrap();
        let patch = PatchSignals::new(signals_generating);
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);

        for _ in 1..=signals.events {
            let mut state = state.write().await;
            let elements = event_entry(&mut state, &Status::Done, "Auto");
            let patch = PatchElements::new(elements).selector("#feed").mode(ElementPatchMode::After);
            let sse_event = patch.write_as_axum_sse_event();
            yield Ok::<_, Infallible>(sse_event);
            tokio::time::sleep(Duration::from_millis(signals.interval)).await;
        }

        let signals_done = serde_json::to_string(&Signals{
            generating: false,
            interval: signals.interval,
            events: signals.events,
        }).unwrap();
        let patch = PatchSignals::new(signals_done);
        let sse_event = patch.write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);
    })
}

async fn event(status: Status, state: Arc<RwLock<State>>) -> impl IntoResponse {
    Sse::new(stream! {
        let mut state = state.write().await;
        let elements = event_entry(&mut state, &status, "Manual");
        let patch = PatchElements::new(elements).selector("#feed").mode(ElementPatchMode::After);
        let sse_event = patch.write_as_axum_sse_event();

        yield Ok::<_, Infallible>(sse_event);
    })
}

fn event_entry(state: &mut State, status: &Status, prefix: &str) -> String {
    state.count.all += 1;
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    match status {
        Status::Done => {
            state.count.done += 1;
            format!(
                "<div id='event-{}' class='text-green-500'>{} [ ✅ Done ] {} event {}</div>",
                state.count.all, timestamp, prefix, state.count.all
            )
        }
        Status::Warn => {
            state.count.warn += 1;
            format!(
                "<div id='event-{}' class='text-yellow-500'>{} [ ⚠️ Warn ] {} event {}</div>",
                state.count.all, timestamp, prefix, state.count.all
            )
        }
        Status::Fail => {
            state.count.fail += 1;
            format!(
                "<div id='event-{}' class='text-red-500'>{} [ ❌ Fail ] {} event {}</div>",
                state.count.all, timestamp, prefix, state.count.all
            )
        }
        Status::Info => {
            state.count.info += 1;
            format!(
                "<div id='event-{}' class='text-blue-500'>{} [ ℹ️ Info ] {} event {}</div>",
                state.count.all, timestamp, prefix, state.count.all
            )
        }
    }
}
