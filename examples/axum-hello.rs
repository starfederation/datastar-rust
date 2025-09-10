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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("hello-world.html"))
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
