use axum::body::Body;
use axum::extract::Json as JsonExtract;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router as AxumRouter};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_disjoint::{api::Deps, health, router, telemetry};
use std::collections::BTreeSet;
use tower::ServiceExt;

const DEAD_URL: &str = "http://127.0.0.1:1";

/// Mock `srvcs-intersection` that ACTUALLY COMPUTES: it reads `{a, b}` from the
/// request and returns `{"a", "b", "result": <sorted distinct ints in both>}`.
/// This is what makes the composition genuinely testable — disjointness is read
/// off a real intersection, not a faked one. A non-integer element produces the
/// same `422` the real leaf does.
async fn spawn_mock_intersection() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|JsonExtract(req): JsonExtract<Value>| async move {
            let to_set = |key: &str| -> Option<BTreeSet<i64>> {
                let mut set = BTreeSet::new();
                for v in req[key].as_array().cloned().unwrap_or_default() {
                    set.insert(v.as_i64()?);
                }
                Some(set)
            };
            match (to_set("a"), to_set("b")) {
                (Some(a), Some(b)) => {
                    let result: Vec<i64> = a.intersection(&b).copied().collect();
                    (
                        StatusCode::OK,
                        Json(json!({ "a": req["a"], "b": req["b"], "result": result })),
                    )
                }
                _ => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": "a and b must be lists of integers" })),
                ),
            }
        }),
    );
    serve(app).await
}

/// Mock `srvcs-intersection` that always answers with a fixed status + body
/// (used to simulate a `500`-inducing body with no `result` array).
async fn spawn_fixed(status: StatusCode, body: Value) -> String {
    let app = AxumRouter::new().route(
        "/",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    serve(app).await
}

async fn serve(app: AxumRouter) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn app(intersection_url: &str) -> axum::Router {
    router(
        telemetry::metrics_handle_for_tests(),
        Deps {
            intersection_url: intersection_url.to_string(),
        },
    )
}

async fn eval(intersection_url: &str, a: Value, b: Value) -> (StatusCode, Value) {
    let res = app(intersection_url)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "a": a, "b": b }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn status_of(uri: &str) -> StatusCode {
    app(DEAD_URL)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

// --- Standard srvcs service surface ---

#[tokio::test]
async fn index_ok() {
    assert_eq!(status_of("/").await, StatusCode::OK);
}

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn metrics_ok() {
    assert_eq!(status_of("/metrics").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

// --- Disjointness, exercised against a REAL computing intersection ---

#[tokio::test]
async fn non_overlapping_sets_are_disjoint() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([1, 2]), json!([3, 4])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], true);
    assert_eq!(body["a"], json!([1, 2]));
    assert_eq!(body["b"], json!([3, 4]));
}

#[tokio::test]
async fn overlapping_sets_are_not_disjoint() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([1, 2]), json!([2, 3])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], false);
}

#[tokio::test]
async fn empty_sets_are_disjoint() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([]), json!([])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], true);
}

#[tokio::test]
async fn one_empty_set_is_disjoint() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([1, 2, 3]), json!([])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], true);
}

#[tokio::test]
async fn identical_sets_are_not_disjoint() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([1, 2, 3]), json!([3, 2, 1])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], false);
}

#[tokio::test]
async fn negatives_handled_via_intersection() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([-2, -1]), json!([-1, 0])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], false);
}

// --- Error / edge cases ---

#[tokio::test]
async fn forwards_422_for_non_integer_element() {
    let inter = spawn_mock_intersection().await;
    let (status, body) = eval(&inter, json!([1, "nope"]), json!([2])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "a and b must be lists of integers");
}

#[tokio::test]
async fn degrades_when_intersection_is_unreachable() {
    let (status, body) = eval(DEAD_URL, json!([1, 2]), json!([3, 4])).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-intersection");
}

#[tokio::test]
async fn server_error_when_intersection_returns_no_array() {
    let inter = spawn_fixed(StatusCode::OK, json!({ "a": [1], "b": [2] })).await;
    let (status, _body) = eval(&inter, json!([1]), json!([2])).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn generates_request_id_when_absent() {
    let res = app(DEAD_URL)
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        res.headers().contains_key("x-request-id"),
        "response must carry a generated x-request-id"
    );
}
