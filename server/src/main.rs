mod app;
mod config;
mod error;
mod modules;
mod response;
mod state;

use sea_orm::{ConnectOptions, Database};
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("server=debug,tower_http=info")),
        )
        .init();

    let config = Config::from_env();

    let mut options = ConnectOptions::new(config.database_url.clone());
    options.max_connections(10).sqlx_logging(false);

    let db = Database::connect(options)
        .await
        .expect("Không kết nối được database");

    let app = app::build_app(AppState { db }, &config);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .unwrap_or_else(|err| panic!("Không mở được {}: {err}", config.bind_addr));

    tracing::info!("Pathnote API đang chạy tại http://{}", config.bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server dừng bất thường");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Không lắng nghe được Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Không lắng nghe được SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Nhận lệnh tắt, đang chờ các request còn lại xử lý xong...");
}
