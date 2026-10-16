use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Set,
};
use server::{
    access_token::AccessTokens, app::build_app, config::Config, entities::users, state::AppState,
    GoogleAuth,
};
use std::sync::Arc;
use tower_sessions_sqlx_store::PostgresStore;

pub struct LearningApp {
    pub api: TestServer,
    pub db: Arc<DatabaseConnection>,
    pub owner_token: String,
    pub other_token: String,
    pub owner_id: i64,
    pub media_root: String,
    admin: DatabaseConnection,
    schema: String,
}
impl LearningApp {
    pub async fn spawn() -> Self {
        Self::spawn_with_collaboration(false).await
    }
    pub async fn spawn_with_collaboration(enabled: bool) -> Self {
        Self::spawn_bridge(enabled, false).await
    }
    pub async fn spawn_bridge(enabled: bool, network: bool) -> Self {
        Self::spawn_config(enabled, network, None).await
    }
    pub async fn spawn_services(url: &str) -> Self {
        Self::spawn_config(false, false, Some((url, url))).await
    }
    pub async fn spawn_service_urls(ipa: &str, translation: &str) -> Self {
        Self::spawn_config(false, false, Some((ipa, translation))).await
    }
    async fn spawn_config(enabled: bool, network: bool, service_url: Option<(&str, &str)>) -> Self {
        dotenvy::dotenv().ok();
        let explicit = std::env::var("TEST_DATABASE_URL").ok();
        let url = explicit.clone().unwrap_or_else(|| {
            std::env::var("DATABASE_URL").expect("Set TEST_DATABASE_URL for PostgreSQL API tests")
        });
        if explicit.is_none() {
            let parsed = reqwest::Url::parse(&url).unwrap();
            assert!(
                matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "::1")),
                "Only local DATABASE_URL may be used as a test fallback"
            );
        }
        let schema = format!("pathnote_learning_test_{}", uuid::Uuid::new_v4().simple());
        let admin = Database::connect(url.clone()).await.unwrap();
        admin
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut options = ConnectOptions::new(url);
        options
            .max_connections(3)
            .set_schema_search_path(&schema)
            .sqlx_logging(false);
        let db = Arc::new(Database::connect(options).await.unwrap());
        Migrator::up(db.as_ref(), None).await.unwrap();
        let store = PostgresStore::new(db.get_postgres_connection_pool().clone())
            .with_schema_name(&schema)
            .unwrap()
            .with_table_name("tower_sessions")
            .unwrap();
        store.migrate().await.unwrap();
        let config = Arc::new(Config {
            collaboration_url: enabled.then(|| "ws://localhost:1234".into()),
            collaboration_secret: enabled.then(|| "test-collaboration-secret-32-characters".into()),
            pronunciation_url: service_url.map(|(ipa, _)| ipa.to_owned()),
            translation_url: service_url.map(|(_, translation)| translation.to_owned()),
            media_root: std::env::temp_dir()
                .join(format!("pathnote_media_test_{}", uuid::Uuid::new_v4()))
                .to_string_lossy()
                .into_owned(),
            google_client_id: "test-client".into(),
            google_client_secret: "test-secret".into(),
            google_redirect_url: "http://localhost:3000/api/v1/auth/google/callback".into(),
            cookie_secure: true,
            bind_addr: "127.0.0.1:3000".into(),
            frontend_url: "https://frontend.example.test".into(),
            database_max_connections: 10,
            database_acquire_timeout_secs: 5,
            database_url: "unused".into(),
            jwt_private_key_path: "unused".into(),
            jwt_issuer: "https://api.example.test".into(),
            jwt_audience: "pathnote-api".into(),
            jwt_key_id: "test-access-key".into(),
        });
        let access = Arc::new(
            AccessTokens::from_pem(
                include_str!("../fixtures/test-only-rsa.pem"),
                config.jwt_issuer.clone(),
                config.jwt_audience.clone(),
                config.jwt_key_id.clone(),
            )
            .unwrap(),
        );
        let owner = seed_user(&db, "owner").await;
        let other = seed_user(&db, "other").await;
        let owner_token = access.issue(owner.id).unwrap();
        let other_token = access.issue(other.id).unwrap();
        let media_root = config.media_root.clone();
        let state = AppState {
            learning_services: Arc::new(server::LearningServices::new(&config).unwrap()),
            db: db.clone(),
            access_tokens: access,
            google: Arc::new(GoogleAuth::new(config.clone()).unwrap()),
            config: config.clone(),
        };
        let builder = if network {
            TestServer::builder().http_transport()
        } else {
            TestServer::builder()
        };
        let api = builder.build(build_app(state, &config, store));
        Self {
            api,
            db,
            owner_token,
            other_token,
            owner_id: owner.id,
            media_root,
            admin,
            schema,
        }
    }
    pub async fn cleanup(self) {
        let _ = tokio::fs::remove_dir_all(&self.media_root).await;
        drop(self.api);
        // HTTP transport stops its server task asynchronously; close the shared pool explicitly.
        self.db.close_by_ref().await.unwrap();
        drop(self.db);
        self.admin
            .execute_unprepared(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .await
            .unwrap();
        self.admin.close().await.unwrap();
    }
}
async fn seed_user(db: &DatabaseConnection, name: &str) -> users::Model {
    let now = chrono::Utc::now().fixed_offset();
    users::ActiveModel {
        name: Set(name.to_owned()),
        email: Set(format!("{name}@example.test")),
        google_id: Set(Some(name.to_owned())),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}
