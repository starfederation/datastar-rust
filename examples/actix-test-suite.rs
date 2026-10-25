mod sdk_test;

use {
    actix_web::{App, HttpResponse, HttpServer, Responder, http::Method, web},
    asynk_strim::{Yielder, stream_fn},
    core::error::Error,
    datastar::{
        DatastarEvent,
        actix::{ReadSignals, Sse},
    },
    sdk_test::TestCase,
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
        // The conformance runner uses GET, POST, and the custom QUERY method.
        // `ReadSignals` reads the `datastar` query parameter for GET/DELETE and
        // the JSON body otherwise. Any other method is rejected with 405.
        let query = Method::from_bytes(b"QUERY").expect("QUERY is a valid method token");
        App::new().service(
            web::resource("/test")
                .route(web::get().to(test))
                .route(web::post().to(test))
                .route(web::method(query).to(test))
                .default_service(web::to(HttpResponse::MethodNotAllowed)),
        )
    })
    .bind(("127.0.0.1", 9200))?
    .run()
    .await?;

    Ok(())
}

async fn test(ReadSignals(test_case): ReadSignals<TestCase>) -> impl Responder {
    Sse::new(stream_fn(
        |mut yielder: Yielder<DatastarEvent>| async move {
            for event in test_case.events {
                yielder.yield_item(event.into_datastar_event()).await;
            }
        },
    ))
}
