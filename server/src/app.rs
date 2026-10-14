use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method},
    middleware::{self, Next},
    response::Response,
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

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
        .merge(modules::media::routes())
        .merge(modules::vocabulary::routes())
        .merge(modules::enrichment::routes())
        .merge(modules::exercises::routes())
        .merge(modules::ai_study::routes())
        .merge(modules::ipa::routes());

    Router::new()
        .nest("/api/v1", api)
        .merge(modules::health::routes())
        .layer(middleware::from_fn(browser_request))
        .fallback(route_not_found)
        .layer(cors(&config.frontend_url))
        .layer(TraceLayer::new_for_http().make_span_with(|request: &Request| tracing::info_span!("http_request", method = %request.method(), path = request.uri().path())))
        .with_state(state)
}

fn cors(frontend_url: &str) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(HeaderValue::from_str(frontend_url).expect("FRONTEND_URL không hợp lệ"))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .allow_credentials(true)
}

async fn browser_request(request: Request, next: Next) -> Result<Response, AppError> {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
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
