mod sdk_test;

use {
    asynk_strim::{Yielder, stream_fn},
    axum::{
        Router,
        response::{IntoResponse, Sse, sse::Event},
        routing::{MethodFilter, on},
    },
    core::{convert::Infallible, error::Error},
    datastar::axum::ReadSignals,
    sdk_test::TestCase,
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

    let app = Router::new().route("/test", on(MethodFilter::GET.or(MethodFilter::POST), test));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9200")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn test(ReadSignals(test_case): ReadSignals<TestCase>) -> impl IntoResponse {
    Sse::new(stream_fn(
        |mut yielder: Yielder<Result<Event, Infallible>>| async move {
            for event in test_case.events {
                let sse_event = event.into_datastar_event().write_as_axum_sse_event();

                yielder.yield_item(Ok(sse_event)).await;
            }
        },
    ))
}
