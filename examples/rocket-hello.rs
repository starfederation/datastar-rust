use {
    core::time::Duration,
    datastar::{prelude::PatchElements, rocket::ReadSignals},
    rocket::{
        get, launch,
        response::{content::RawHtml, stream::EventStream},
        routes,
        serde::Deserialize,
    },
};

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index, hello_world])
}

#[get("/")]
fn index() -> RawHtml<&'static str> {
    RawHtml(include_str!("hello-world.html"))
}

const MESSAGE: &str = "Hello, world!";

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Signals {
    delay: u64,
}

#[get("/hello-world")]
fn hello_world(signals: ReadSignals<Signals>) -> EventStream![] {
    EventStream! {
        for i in 0..MESSAGE.len() {
            let elements = format!("<div id='message'>{}</div>", &MESSAGE[0..i + 1]);
            let patch = PatchElements::new(elements);
            let sse_event = patch.write_as_rocket_sse_event();

            yield sse_event;

            rocket::tokio::time::sleep(Duration::from_millis(signals.0.delay)).await;
        }
    }
}
