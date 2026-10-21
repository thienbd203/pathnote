use axum::http::StatusCode;
use axum::{
    extract::{Request, State},
    http::{header, Extensions, HeaderMap, HeaderValue, Method, Version},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use std::time::Duration;
use tower_http::{
    compression::{
        predicate::{DefaultPredicate, Predicate},
        CompressionLayer,
    },
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{
    auth::Backend,
    config::Config,
    error::{route_not_found, AppError},
    modules,
    state::AppState,
};
use axum_login::AuthManagerLayerBuilder;
use tower_sessions::{cookie::SameSite, Expiry, SessionManagerLayer, SessionStore};

pub fn build_app<S: SessionStore + Clone>(state: AppState, config: &Config, store: S) -> Router {
    let sessions = SessionManagerLayer::new(store)
        .with_name("pathnote_session")
        .with_http_only(true)
        .with_secure(config.cookie_secure)
        .with_same_site(SameSite::Lax)
        .with_path("/api/v1/auth")
        .with_expiry(Expiry::OnInactivity(time::Duration::minutes(10)));
    let auth = AuthManagerLayerBuilder::new(
        Backend {
            db: state.db.clone(),
            google: state.google.clone(),
        },
        sessions,
    )
    .build();
    let api = Router::new()
        .merge(modules::auth::routes())
        .layer(auth)
        .layer(middleware::from_fn_with_state(state.clone(), cookie_origin))
        .merge(modules::auth::access_routes())
        .merge(modules::places::routes())
        .merge(modules::courses::routes())
        .merge(modules::sessions::routes())
        .merge(modules::notes::routes(state.clone()))
        .merge(modules::vocabulary::routes())
        .merge(modules::enrichment::routes())
        .merge(modules::exercises::routes())
        .merge(modules::ai_study::routes())
        .merge(modules::settings::routes())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        // Uploads and imports need a separate budget, outside the default timeout.
        .merge(
            modules::media::routes().layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(120),
            )),
        )
        .merge(modules::ipa::routes().layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(120),
        )));

    Router::new()
        .nest("/api/v1", api)
        .merge(modules::health::routes())
        .fallback(route_not_found)
        .method_not_allowed_fallback(|| async { AppError::MethodNotAllowed })
        .layer(middleware::from_fn(browser_request))
        .layer(cors(&config.frontend_url))
        .layer(CompressionLayer::new().compress_when(DefaultPredicate::new().and(compressible_content)))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http().make_span_with(|request: &Request| tracing::info_span!("http_request", method = %request.method(), path = request.uri().path(), request_id = request.headers().get("x-request-id").and_then(|value| value.to_str().ok()).unwrap_or(""))))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}

fn compressible_content(_: StatusCode, _: Version, headers: &HeaderMap, _: &Extensions) -> bool {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    content_type == "application/json"
        || content_type.starts_with("text/")
        || content_type.ends_with("+json")
}

fn cors(frontend_url: &str) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(HeaderValue::from_str(frontend_url).expect("FRONTEND_URL không hợp lệ"))
        .allow_methods([
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::RANGE,
        ])
        .expose_headers([header::HeaderName::from_static("x-request-id")])
        .allow_credentials(true)
}

async fn browser_request(request: Request, next: Next) -> Result<Response, AppError> {
    let mut response = next.run(request).await;
    if response.status() == StatusCode::REQUEST_TIMEOUT {
        response = AppError::RequestTimeout.into_response();
    }
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert_with(|| HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    Ok(response)
}

async fn cookie_origin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        if origin != Some(state.config.frontend_url.as_str()) {
            return Err(AppError::Forbidden);
        }
    }
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compression_is_limited_to_json_and_text() {
        for (mime, expected) in [
            ("application/json; charset=utf-8", true),
            ("text/plain", true),
            ("application/problem+json", true),
            ("application/pdf", false),
            ("application/zip", false),
            (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                false,
            ),
            ("image/png", false),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
            assert_eq!(
                compressible_content(
                    StatusCode::OK,
                    Version::HTTP_11,
                    &headers,
                    &Extensions::new()
                ),
                expected,
                "{mime}"
            );
        }
    }
}
