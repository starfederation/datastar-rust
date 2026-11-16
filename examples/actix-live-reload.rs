use actix_sse::{Event, Sse};
use actix_web::{
    App, HttpServer, Responder,
    web::{self, Html},
};
use asynk_strim::{Yielder, stream_fn};
use datastar::{actix_web::ReadSignals, prelude::PatchElements};
use serde::Deserialize;
use std::time::Duration;
use {
    std::error::Error,
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
};

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
        let app = App::new()
            .route("/", web::get().to(index))
            .route("/hello-world", web::get().to(hello_world));

        #[cfg(debug_assertions)]
        let app = app.route("/hotreload", web::get().to(hotreload));

        app
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await?;

    Ok(())
}

const INDEX_HTML: &str = include_str!("hello-world.html");

#[cfg(not(debug_assertions))]
async fn index() -> Html {
    Html::new(INDEX_HTML)
}

#[cfg(debug_assertions)]
async fn index() -> Html {
    static MOD_INDEX_HTML: std::sync::LazyLock<&'static str> = std::sync::LazyLock::new(|| {
        Box::new(INDEX_HTML.replace(
            r##"<!-- hot reload -->"##,
            r##"
    <div id="hotreload" data-on-load="@get('/hotreload', {retryMaxCount: 1000,retryInterval:20, retryMaxWaitMs:200})" class="text-yellow-500">
        <p>a minimal implementation of dev-only live reload added into Actix Web hello example</p>
    </div>"##
        )).leak()
    });
    Html::new(&**MOD_INDEX_HTML)
}

#[cfg(debug_assertions)]
async fn hotreload() -> impl Responder {
    use std::sync::atomic;

    // NOTE
    // This only works if you develop with a single tab open only,
    // in case you are testing with multiple UA's / Tabs at once
    // you will need to expand this implementation by for example
    // tracking against a date or version stored in a cookie
    // or by some other means.

    use asynk_strim::Yielder;
    use datastar::prelude::ExecuteScript;
    static ONCE: atomic::AtomicBool = atomic::AtomicBool::new(false);

    Sse::from_infallible_stream(stream_fn(|mut yielder: Yielder<Event>| async move {
        if !ONCE.swap(true, atomic::Ordering::SeqCst) {
            let script = ExecuteScript::new("window.location.reload()");
            let event = script.into();
            yielder.yield_item(event).await;
        }
        std::future::pending().await
    }))
}

const MESSAGE: &str = "Hello, world!";

#[derive(Deserialize)]
pub struct Signals {
    pub delay: u64,
}

async fn hello_world(ReadSignals(signals): ReadSignals<Signals>) -> impl Responder {
    Sse::from_infallible_stream(stream_fn(move |mut yielder: Yielder<Event>| async move {
        for i in 0..MESSAGE.len() {
            let elements = format!("<div id='message'>{}</div>", &MESSAGE[0..i + 1]);
            let event = PatchElements::new(elements).into();

            yielder.yield_item(event).await;

            tokio::time::sleep(Duration::from_millis(signals.delay)).await;
        }
    }))
}
