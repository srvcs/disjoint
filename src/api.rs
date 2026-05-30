use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{OpenApi, ToSchema};

use crate::client::{self, DepError};

pub const SERVICE: &str = "srvcs-disjoint";
pub const CONCERN: &str = "sets: do the sets share no elements";
pub const DEPENDS_ON: &[&str] = &["srvcs-intersection"];

/// Dependency endpoints, injected as router state so tests can point them at
/// mock services.
#[derive(Clone)]
pub struct Deps {
    pub intersection_url: String,
}

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub service: &'static str,
    pub concern: &'static str,
    pub depends_on: Vec<&'static str>,
}

/// `GET /` — service identity (srvcs service standard).
#[utoipa::path(get, path = "/", responses((status = 200, body = Info)))]
pub async fn index() -> Json<Info> {
    Json(Info {
        service: SERVICE,
        concern: CONCERN,
        depends_on: DEPENDS_ON.to_vec(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct EvalRequest {
    /// The first set, as a list of integers. Validation of the elements is
    /// delegated to `srvcs-intersection`.
    #[schema(value_type = Object)]
    pub a: Vec<Value>,
    /// The second set, as a list of integers. Validation of the elements is
    /// delegated to `srvcs-intersection`.
    #[schema(value_type = Object)]
    pub b: Vec<Value>,
}

#[derive(Serialize, ToSchema)]
pub struct DisjointResponse {
    #[schema(value_type = Object)]
    pub a: Vec<Value>,
    #[schema(value_type = Object)]
    pub b: Vec<Value>,
    pub result: bool,
}

fn degraded(dependency: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "dependency unavailable", "dependency": dependency })),
    )
        .into_response()
}

/// Forward a dependency's response verbatim (used to propagate `422` for invalid
/// input, so disjoint reports the same rejection `srvcs-intersection` did).
fn forward(status: u16, body: Value) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    (code, Json(body)).into_response()
}

/// Ask `srvcs-intersection` for the intersection of `a` and `b`, returning its
/// `result` array.
///
/// Maps the dependency's failures to the response this service should return:
/// `503` if it is unreachable, the forwarded `422` if intersection rejects an
/// element (e.g. a non-integer), and a generic `500` if it returns an unusable
/// body.
async fn ask_intersection(url: &str, a: &[Value], b: &[Value]) -> Result<Vec<Value>, Response> {
    let body = json!({ "a": a, "b": b });
    match client::call(url, &body).await {
        Err(DepError::Unreachable) => Err(degraded("srvcs-intersection")),
        Ok((200, body)) => match body.get("result").and_then(Value::as_array) {
            Some(arr) => Ok(arr.clone()),
            None => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "srvcs-intersection returned no array result" })),
            )
                .into_response()),
        },
        // Bad element (e.g. not an integer) — intersection judged it; forward it.
        Ok((422, body)) => Err(forward(422, body)),
        Ok(_) => Err(degraded("srvcs-intersection")),
    }
}

/// `POST /` — are the two sets disjoint (do they share no elements)?
///
/// This service does no set logic of its own. It asks `srvcs-intersection` for
/// the intersection of `a` and `b`, then reports whether that intersection is
/// empty. If intersection rejects an element the `422` is forwarded; if it is
/// unreachable this service reports itself degraded rather than guessing.
#[utoipa::path(
    post,
    path = "/",
    request_body = EvalRequest,
    responses(
        (status = 200, body = DisjointResponse),
        (status = 422, description = "an element is not a valid integer (forwarded from srvcs-intersection)"),
        (status = 500, description = "srvcs-intersection returned an unusable response"),
        (status = 503, description = "the srvcs-intersection dependency is unavailable")
    )
)]
pub async fn evaluate(State(deps): State<Deps>, Json(req): Json<EvalRequest>) -> Response {
    let inter = match ask_intersection(&deps.intersection_url, &req.a, &req.b).await {
        Ok(inter) => inter,
        Err(resp) => return resp,
    };
    let result = inter.is_empty();
    (
        StatusCode::OK,
        Json(json!({ "a": req.a, "b": req.b, "result": result })),
    )
        .into_response()
}

#[derive(OpenApi)]
#[openapi(
    paths(index, evaluate),
    components(schemas(Info, EvalRequest, DisjointResponse))
)]
pub struct ApiDoc;

/// Serve OpenAPI document
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_routes() {
        let doc = ApiDoc::openapi();
        let root = doc.paths.paths.get("/").expect("path / present");
        assert!(root.get.is_some());
        assert!(root.post.is_some());
    }

    #[tokio::test]
    async fn index_reports_dependency() {
        let Json(info) = index().await;
        assert_eq!(info.service, "srvcs-disjoint");
        assert_eq!(info.concern, "sets: do the sets share no elements");
        assert_eq!(info.depends_on, vec!["srvcs-intersection"]);
    }
}
