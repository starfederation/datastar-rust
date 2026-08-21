use {
    asynk_strim::{Yielder, stream_fn},
    axum::{
        Router,
        extract::State,
        response::{
            Html, IntoResponse, Sse,
            sse::{Event, KeepAlive},
        },
        routing::get,
    },
    core::{convert::Infallible, error::Error, time::Duration},
    datastar::prelude::PatchElements,
    tokio::sync::watch,
};

#[derive(Clone)]
struct Dashboard {
    wind: watch::Receiver<u64>,
    temperature: watch::Receiver<u64>,
    humidity: watch::Receiver<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (wind_tx, wind) = watch::channel(12);
    let (temperature_tx, temperature) = watch::channel(21);
    let (humidity_tx, humidity) = watch::channel(45);

    spawn_counter(wind_tx, 12, 1, Duration::from_secs(1));
    spawn_counter(temperature_tx, 21, 1, Duration::from_secs(3));
    spawn_counter(humidity_tx, 45, 2, Duration::from_secs(2));

    let app = Router::new()
        .route("/", get(index))
        .route("/dashboard", get(dashboard))
        .with_state(Dashboard {
            wind,
            temperature,
            humidity,
        });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_counter(sender: watch::Sender<u64>, mut value: u64, step: u64, interval: Duration) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            value += step;
            if sender.send(value).is_err() {
                break;
            }
        }
    });
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn dashboard(State(state): State<Dashboard>) -> impl IntoResponse {
    let mut wind = state.wind;
    let mut temperature = state.temperature;
    let mut humidity = state.humidity;

    Sse::new(stream_fn(
        move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
            yielder
                .yield_item(Ok(render_dashboard(
                    &mut wind,
                    &mut temperature,
                    &mut humidity,
                )
                .write_as_axum_sse_event()))
                .await;

            'updates: loop {
                let patch = tokio::select! {
                    result = wind.changed() => {
                        if result.is_err() {
                            break 'updates;
                        }
                        PatchElements::new(format!(
                            "<dd id='wind'>{} km/h</dd>",
                            *wind.borrow(),
                        ))
                    }
                    result = temperature.changed() => {
                        if result.is_err() {
                            break 'updates;
                        }
                        PatchElements::new(format!(
                            "<dd id='temperature'>{} °C</dd>",
                            *temperature.borrow(),
                        ))
                    }
                    result = humidity.changed() => {
                        if result.is_err() {
                            break 'updates;
                        }
                        PatchElements::new(format!(
                            "<dd id='humidity'>{}%</dd>",
                            *humidity.borrow(),
                        ))
                    }
                };

                yielder
                    .yield_item(Ok(patch.write_as_axum_sse_event()))
                    .await;
            }
        },
    ))
    .keep_alive(KeepAlive::default())
}

fn render_dashboard(
    wind: &mut watch::Receiver<u64>,
    temperature: &mut watch::Receiver<u64>,
    humidity: &mut watch::Receiver<u64>,
) -> PatchElements {
    PatchElements::new(format!(
        concat!(
            "<dl id='dashboard'>",
            "<dt>Wind</dt><dd id='wind'>{} km/h</dd>",
            "<dt>Temperature</dt><dd id='temperature'>{} °C</dd>",
            "<dt>Humidity</dt><dd id='humidity'>{}%</dd>",
            "</dl>",
        ),
        *wind.borrow_and_update(),
        *temperature.borrow_and_update(),
        *humidity.borrow_and_update(),
    ))
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8">
        <title>Datastar watch channels</title>
        <script type="module" src="https://cdn.jsdelivr.net/gh/starfederation/datastar@v1.0.2/bundles/datastar.js"></script>
    </head>
    <body>
        <main data-init="@get('/dashboard')">
            <h1>Live dashboard</h1>
            <dl id="dashboard"><dt>Status</dt><dd>Connecting…</dd></dl>
        </main>
    </body>
</html>
"#;
