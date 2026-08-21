mod sdk_test;

use {
    asynk_strim::{Yielder, stream_fn},
    core::{convert::Infallible, error::Error},
    datastar::warp::{ReadSignals, read_signals},
    sdk_test::TestCase,
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
    warp::{Filter, filters::sse::Event},
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

    let test = warp::path("test")
        .and(warp::get().or(warp::post()).unify())
        .and(read_signals::<TestCase>())
        .map(|ReadSignals(test_case): ReadSignals<TestCase>| {
            let stream = stream_fn(
                |mut yielder: Yielder<Result<Event, Infallible>>| async move {
                    for event in test_case.events {
                        let sse_event = event.into_datastar_event().write_as_warp_sse_event();

                        yielder.yield_item(Ok(sse_event)).await;
                    }
                },
            );
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    tracing::debug!("listening on 127.0.0.1:9200");
    warp::serve(test).run(([127, 0, 0, 1], 9200)).await;

    Ok(())
}
