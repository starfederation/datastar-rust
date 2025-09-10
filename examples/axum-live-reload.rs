use {
    asynk_strim::{Yielder, stream_fn},
    axum::{
        Router,
        response::{Html, IntoResponse, Sse, sse::Event},
        routing::get,
    },
    core::{convert::Infallible, error::Error, time::Duration},
    datastar::{axum::ReadSignals, prelude::PatchElements},
    serde::Deserialize,
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
        .route("/hello-world", get(hello_world));

    #[cfg(debug_assertions)]
    let app = app.route("/hotreload", get(hotreload));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

const INDEX_HTML: &str = include_str!("hello-world.html");

#[cfg(not(debug_assertions))]
async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

#[cfg(debug_assertions)]
async fn index() -> Html<&'static str> {
    static MOD_INDEX_HTML: std::sync::LazyLock<&'static str> = std::sync::LazyLock::new(|| {
        Box::new(INDEX_HTML.replace(
            r##"<!-- hot reload -->"##,
            r##"
    <div id="hotreload" data-on-load="@get('/hotreload', {retryMaxCount: 1000,retryInterval:20, retryMaxWaitMs:200})" class="text-yellow-500">
        <p>a minimal implementation of dev-only live reload added into axum hello example</p>
    </div>"##
        )).leak()
    });
    Html(&MOD_INDEX_HTML)
}

#[cfg(debug_assertions)]
async fn hotreload() -> impl IntoResponse {
    use std::sync::atomic;

    // NOTE
    // This only works if you develop with a single tab open only,
    // in case you are testing with multiple UA's / Tabs at once
    // you will need to expand this implementation by for example
    // tracking against a date or version stored in a cookie
    // or by some other means.

    use asynk_strim::Yielder;
    use axum::response::sse;
    use datastar::prelude::ExecuteScript;
    static ONCE: atomic::AtomicBool = atomic::AtomicBool::new(false);

    Sse::new(stream_fn(
        |mut yielder: Yielder<Result<sse::Event, Infallible>>| async move {
            if !ONCE.swap(true, atomic::Ordering::SeqCst) {
                let script = ExecuteScript::new("window.location.reload()");
                let sse_event = script.write_as_axum_sse_event();
                yielder.yield_item(Ok(sse_event)).await;
            }
            std::future::pending().await
        },
    ))
}

const MESSAGE: &str = "Hello, world!";

#[derive(Deserialize)]
pub struct Signals {
    pub delay: u64,
}

async fn hello_world(ReadSignals(signals): ReadSignals<Signals>) -> impl IntoResponse {
    Sse::new(stream_fn(
        move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
            for i in 0..MESSAGE.len() {
                let elements = format!("<div id='message'>{}</div>", &MESSAGE[0..i + 1]);
                let patch = PatchElements::new(elements);
                let sse_event = patch.write_as_axum_sse_event();

                yielder.yield_item(Ok(sse_event)).await;

                tokio::time::sleep(Duration::from_millis(signals.delay)).await;
            }
        },
    ))
}
