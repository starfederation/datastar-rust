//! Integration tests for the Actix-Web Datastar integration.

use {
    actix_web::{
        App, HttpResponse, Responder,
        body::to_bytes,
        dev::ServiceResponse,
        http::{
            Method, StatusCode,
            header::{CACHE_CONTROL, CONTENT_TYPE},
        },
        test, web,
    },
    datastar::{
        actix::{ReadSignals, Sse},
        prelude::{ExecuteScript, PatchElements, PatchSignals},
    },
    futures_util::stream,
    serde::{Deserialize, Serialize},
};

/// Asserts the common SSE response headers are present on an SSE response.
fn assert_sse_headers(resp: &ServiceResponse) {
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_TYPE).unwrap(),
        "text/event-stream"
    );
    assert_eq!(resp.headers().get(CACHE_CONTROL).unwrap(), "no-cache");
}

// =============================================================================
// Write side tests
// =============================================================================

#[actix_web::test]
async fn patch_elements_single_event() {
    async fn handler() -> impl Responder {
        Sse::from(PatchElements::new("<div id=\"target\">Hello</div>"))
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_sse_headers(&resp);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let body_str = std::str::from_utf8(&body).unwrap();

    assert!(body_str.starts_with("event: datastar-patch-elements\n"));
    assert!(body_str.contains("elements <div id=\"target\">Hello</div>"));
    assert!(body_str.ends_with("\n\n"));
}

#[actix_web::test]
async fn patch_signals_single_event() {
    async fn handler() -> impl Responder {
        Sse::from(PatchSignals::new("{ \"foo\": true }"))
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_sse_headers(&resp);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let body_str = std::str::from_utf8(&body).unwrap();

    assert!(body_str.starts_with("event: datastar-patch-signals\n"));
    assert!(body_str.contains("signals { \"foo\": true }"));
    assert!(body_str.ends_with("\n\n"));
}

#[actix_web::test]
async fn execute_script_single_event() {
    async fn handler() -> impl Responder {
        Sse::from(ExecuteScript::new("console.log('hello')"))
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_sse_headers(&resp);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let body_str = std::str::from_utf8(&body).unwrap();

    // ExecuteScript produces a PatchElements event wrapping a <script> tag
    assert!(body_str.starts_with("event: datastar-patch-elements\n"));
    assert!(body_str.contains("console.log('hello')"));
    assert!(body_str.contains("<script"));
    assert!(body_str.contains("</script>"));
    assert!(body_str.ends_with("\n\n"));
}

#[actix_web::test]
async fn streaming_multiple_events() {
    async fn handler() -> impl Responder {
        let events = vec![
            PatchElements::new("<div>one</div>").as_datastar_event(),
            PatchElements::new("<div>two</div>").as_datastar_event(),
        ];
        Sse::new(stream::iter(events))
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_sse_headers(&resp);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let body_str = std::str::from_utf8(&body).unwrap();

    // Should contain two distinct SSE events, each terminated by \n\n
    let event_count = body_str.matches("event: datastar-patch-elements").count();
    assert_eq!(event_count, 2);
    assert!(body_str.contains("elements <div>one</div>"));
    assert!(body_str.contains("elements <div>two</div>"));
}

#[actix_web::test]
async fn borrowed_conversions_match_owned() {
    async fn render(sse: Sse) -> String {
        let req = test::TestRequest::get().to_http_request();
        let body = to_bytes(sse.respond_to(&req).into_body()).await.unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    let elements = PatchElements::new("<div>hi</div>").id("e1");
    let signals = PatchSignals::new("{\"a\":1}").id("s1");
    let script = ExecuteScript::new("console.log(1)").id("x1");
    let event = elements.as_datastar_event();

    assert_eq!(
        render(Sse::from(&elements)).await,
        render(Sse::from(elements)).await
    );
    assert_eq!(
        render(Sse::from(&signals)).await,
        render(Sse::from(signals)).await
    );
    assert_eq!(
        render(Sse::from(&script)).await,
        render(Sse::from(script)).await
    );
    assert_eq!(
        render(Sse::from(&event)).await,
        render(Sse::from(event)).await
    );
}

// =============================================================================
// Read side tests
// =============================================================================

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct TestSignals {
    name: String,
    count: i64,
}

#[actix_web::test]
async fn read_signals_from_post_body() {
    async fn handler(ReadSignals(signals): ReadSignals<TestSignals>) -> impl Responder {
        HttpResponse::Ok().json(serde_json::json!({
            "name": signals.name,
            "count": signals.count,
        }))
    }

    let app = test::init_service(App::new().route("/", web::post().to(handler))).await;
    let req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"name": "datastar", "count": 42}))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let parsed: TestSignals = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed.name, "datastar");
    assert_eq!(parsed.count, 42);
}

#[actix_web::test]
async fn read_signals_from_get_query_param() {
    async fn handler(ReadSignals(signals): ReadSignals<TestSignals>) -> impl Responder {
        HttpResponse::Ok().json(serde_json::json!({
            "name": signals.name,
            "count": signals.count,
        }))
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;

    // GET with datastar query param: the value is a JSON-encoded string
    let json_signals = serde_json::to_string(&TestSignals {
        name: "query".to_owned(),
        count: 7,
    })
    .unwrap();
    let encoded = urlencoding::encode(&json_signals);

    let req = test::TestRequest::get()
        .uri(&format!("/?datastar={encoded}"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let parsed: TestSignals = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed.name, "query");
    assert_eq!(parsed.count, 7);
}

#[actix_web::test]
async fn read_signals_rejects_invalid_post_body() {
    async fn handler(ReadSignals(signals): ReadSignals<TestSignals>) -> impl Responder {
        HttpResponse::Ok().json(serde_json::json!({"name": signals.name}))
    }

    let app = test::init_service(App::new().route("/", web::post().to(handler))).await;
    let req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"not_name": "missing fields"}))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn read_signals_from_delete_query_param() {
    async fn handler(ReadSignals(signals): ReadSignals<TestSignals>) -> impl Responder {
        HttpResponse::Ok().json(serde_json::json!({
            "name": signals.name,
            "count": signals.count,
        }))
    }

    let app = test::init_service(App::new().route("/", web::delete().to(handler))).await;

    // Datastar sends DELETE signals in the query parameter, like GET.
    let json_signals = serde_json::to_string(&TestSignals {
        name: "deleted".to_owned(),
        count: 8,
    })
    .unwrap();
    let encoded = urlencoding::encode(&json_signals);

    let req = test::TestRequest::delete()
        .uri(&format!("/?datastar={encoded}"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body()).await.unwrap();
    let parsed: TestSignals = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed.name, "deleted");
    assert_eq!(parsed.count, 8);
}

#[actix_web::test]
async fn read_signals_missing_query_param_yields_none_for_option() {
    async fn handler(ReadSignals(maybe): ReadSignals<Option<TestSignals>>) -> impl Responder {
        match maybe {
            Some(_) => HttpResponse::Ok().body("signals"),
            None => HttpResponse::Ok().body("no signals"),
        }
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;

    // A plain page load carries no `datastar` parameter; the extractor
    // deserializes `null`, so `ReadSignals<Option<T>>` yields `None`.
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body()).await.unwrap();
    assert_eq!(body, "no signals");
}

#[actix_web::test]
async fn read_signals_get_query_param_populates_option() {
    async fn handler(ReadSignals(maybe): ReadSignals<Option<TestSignals>>) -> impl Responder {
        match maybe {
            Some(_) => HttpResponse::Ok().body("signals"),
            None => HttpResponse::Ok().body("no signals"),
        }
    }

    let app = test::init_service(App::new().route("/", web::get().to(handler))).await;

    let json_signals = serde_json::to_string(&TestSignals {
        name: "query".to_owned(),
        count: 7,
    })
    .unwrap();
    let encoded = urlencoding::encode(&json_signals);

    let req = test::TestRequest::get()
        .uri(&format!("/?datastar={encoded}"))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body()).await.unwrap();
    assert_eq!(body, "signals");
}

#[actix_web::test]
async fn read_signals_from_custom_query_method_body() {
    async fn handler(ReadSignals(signals): ReadSignals<TestSignals>) -> HttpResponse {
        HttpResponse::Ok().json(signals)
    }

    let query = Method::from_bytes(b"QUERY").unwrap();
    let app =
        test::init_service(App::new().route("/", web::method(query.clone()).to(handler))).await;

    // QUERY reads the JSON body; a conflicting `datastar` URL parameter is ignored.
    let req = test::TestRequest::default()
        .method(query)
        .uri("/?datastar=%7B%22name%22%3A%22url%22%2C%22count%22%3A0%7D")
        .set_json(TestSignals {
            name: "body".to_owned(),
            count: 7,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let got: TestSignals = test::read_body_json(resp).await;
    assert_eq!(
        got,
        TestSignals {
            name: "body".to_owned(),
            count: 7,
        }
    );
}
