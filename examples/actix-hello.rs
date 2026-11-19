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

#[actix_web::main]
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
            .route("/hello-world", web::get().to(hello_world))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await?;

    Ok(())
}

async fn index() -> Html {
    Html::new(include_str!("hello-world.html"))
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
