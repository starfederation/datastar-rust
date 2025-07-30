use {
    core::time::Duration,
    datastar::prelude::PatchElements,
    rocket::{
        Shutdown, State, get, launch,
        response::{content::RawHtml, stream::Event, stream::EventStream},
        routes,
        serde::{Deserialize, json::Json},
        tokio::sync::watch,
    },
};

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, hello_world, set_delay])
        .manage(watch::channel(Signals { delay: 400 }))
}

#[get("/")]
fn index() -> RawHtml<&'static str> {
    RawHtml(include_str!("hello-world-channel.html"))
}

const MESSAGE: &str = "Hello, world!";

#[derive(Deserialize, Clone, Debug, Copy)]
#[serde(crate = "rocket::serde")]
struct Signals {
    delay: u64,
}

#[get("/set-delay?<datastar>")]
fn set_delay(
    datastar: Json<Signals>,
    signals_channel: &State<(watch::Sender<Signals>, watch::Receiver<Signals>)>,
) {
    let (tx, _) = &**signals_channel;
    let _ = tx.send(datastar.into_inner());
}

#[get("/hello-world")]
fn hello_world(
    signals_channel: &State<(watch::Sender<Signals>, watch::Receiver<Signals>)>,
    mut shutdown: Shutdown,
) -> EventStream![Event + '_] {
    let mut rx = signals_channel.inner().1.clone();

    EventStream! {
        'animation: loop {
            let delay = rx.borrow().delay;

            for i in 0..=MESSAGE.len() {
                let elements = format!("<div id='message'>{}</div>", &MESSAGE[0..i]);
                let patch = PatchElements::new(elements);
                yield patch.write_as_rocket_sse_event();

                tokio::select! {
                    biased;
                    _ = &mut shutdown => {
                        break 'animation;
                    }
                    _ = rocket::tokio::time::sleep(Duration::from_millis(delay)) => {
                    }

                    result = rx.changed() => {
                        if result.is_err() {
                            break 'animation;
                        }
                        continue 'animation;
                    }
                }
            }

            tokio::select! {
                biased;
                _ = &mut shutdown => break 'animation,
                result = rx.changed() => {
                    if result.is_err() {
                        break 'animation;
                    }
                }
            }
        }
    }
}
