use {
    asynk_strim::{Yielder, stream_fn},
    core::{convert::Infallible, error::Error, time::Duration},
    datastar::{
        prelude::PatchElements,
        warp::{ReadSignals, read_signals},
    },
    serde::Deserialize,
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

    let index = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::html(include_str!("hello-world.html")));

    let hello_world = warp::path("hello-world")
        .and(warp::get())
        .and(read_signals::<Signals>())
        .map(|ReadSignals(signals): ReadSignals<Signals>| {
            let stream = stream_fn(
                move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
                    for i in 0..MESSAGE.len() {
                        let elements = format!("<div id='message'>{}</div>", &MESSAGE[0..i + 1]);
                        let patch = PatchElements::new(elements);
                        let sse_event = patch.write_as_warp_sse_event();

                        yielder.yield_item(Ok(sse_event)).await;
                        tokio::time::sleep(Duration::from_millis(signals.delay)).await;
                    }
                },
            );
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    let routes = index.or(hello_world);

    tracing::debug!("listening on 127.0.0.1:3000");
    warp::serve(routes).run(([127, 0, 0, 1], 3000)).await;

    Ok(())
}

const MESSAGE: &str = "Hello, world!";

#[derive(Deserialize)]
pub struct Signals {
    pub delay: u64,
}
