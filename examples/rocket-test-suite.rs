mod sdk_test;

use {
    datastar::rocket::ReadSignals,
    rocket::{
        get, post,
        response::stream::{Event, EventStream},
        routes,
    },
    sdk_test::TestCase,
};

#[rocket::main]
async fn main() -> Result<(), Box<rocket::Error>> {
    let config = rocket::Config::figment().merge(("port", 9200));
    rocket::custom(config)
        .mount("/", routes![get_test, post_test])
        .launch()
        .await
        .map_err(Box::new)?;

    Ok(())
}

#[get("/test")]
fn get_test(test_case: ReadSignals<TestCase>) -> EventStream![Event] {
    test_stream(test_case.0)
}

#[post("/test", data = "<test_case>")]
fn post_test(test_case: ReadSignals<TestCase>) -> EventStream![Event] {
    test_stream(test_case.0)
}

fn test_stream(test_case: TestCase) -> EventStream![Event] {
    let stream = EventStream! {
        for event in test_case.events {
            yield event.into_datastar_event().write_as_rocket_sse_event();
        }
    };

    stream.heartbeat(None)
}
