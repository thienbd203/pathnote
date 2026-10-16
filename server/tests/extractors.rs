use axum::{
    routing::{get, post},
    Json, Router,
};
use axum_test::TestServer;
use serde::Deserialize;
use server::{
    extract::{AppJson, AppPath, AppQuery},
    AppError,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    count: u64,
}

#[tokio::test]
async fn rejection_responses_are_json() {
    let app = Router::new()
        .route(
            "/body",
            post(|AppJson(input): AppJson<Input>| async move { Json(input.count) }),
        )
        .route(
            "/query",
            get(|AppQuery(input): AppQuery<Input>| async move { Json(input.count) }),
        )
        .route(
            "/path/{id}",
            get(|AppPath(id): AppPath<i64>| async move { Json(id) }),
        )
        .method_not_allowed_fallback(|| async { AppError::MethodNotAllowed });
    let api = TestServer::new(app);
    for (response, status, field) in [
        (
            api.post("/body")
                .content_type("application/json")
                .text("{")
                .await,
            422,
            Some("body"),
        ),
        (api.get("/query?count=bad").await, 422, Some("query")),
        (api.get("/path/abc").await, 404, None),
        (api.delete("/body").await, 405, None),
    ] {
        response.assert_status(axum::http::StatusCode::from_u16(status).unwrap());
        let body = response.json::<serde_json::Value>();
        assert!(body["message"].is_string());
        if let Some(field) = field {
            assert!(body["errors"][field].is_array());
        }
    }
}
