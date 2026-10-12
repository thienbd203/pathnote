use sea_orm::{ConnectOptions, Database};
use tracing_subscriber::EnvFilter;

use server::{config::Config, state::AppState, GoogleAuth};
use std::sync::Arc;
use tower_sessions::ExpiredDeletion;
use tower_sessions_sqlx_store::PostgresStore;

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

    let config = Arc::new(config);
    let db = Arc::new(db);
    let session_store = PostgresStore::new(db.get_postgres_connection_pool().clone())
        .with_schema_name("public")
        .expect("Tên schema không hợp lệ")
        .with_table_name("tower_sessions")
        .expect("Tên bảng không hợp lệ");
    session_store
        .migrate()
        .await
        .expect("Không khởi tạo được bảng lưu phiên");
    let cleanup_store = session_store.clone();
    let cleanup = tokio::spawn(async move {
        if let Err(error) = cleanup_store
            .continuously_delete_expired(std::time::Duration::from_secs(60))
            .await
        {
            tracing::error!(%error, "Dọn phiên hết hạn đã dừng");
        }
    });
    let pem = std::fs::read_to_string(&config.jwt_private_key_path)
        .expect("Không đọc được khóa ký access token");
    let access_tokens = server::access_token::AccessTokens::from_pem(
        &pem,
        config.jwt_issuer.clone(),
        config.jwt_audience.clone(),
        config.jwt_key_id.clone(),
    )
    .expect("Không khởi tạo được access token");
    let state = AppState {
        access_tokens: Arc::new(access_tokens),
        db,
        google: Arc::new(
            GoogleAuth::new(config.clone()).expect("Không khởi tạo được Google client"),
        ),
        config: config.clone(),
    };

    let app = server::app::build_app(state, &config, session_store);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .unwrap_or_else(|err| panic!("Không mở được {}: {err}", config.bind_addr));

    tracing::info!("Pathnote API đang chạy tại http://{}", config.bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server dừng bất thường");
    cleanup.abort();
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
