//! Explicitly launched, isolated PostgreSQL harness for the browser contract suite.
//! This module is only in the test binary; it adds no login bypass to the server.
use super::learning_helpers::LearningApp;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use server::entities::users;
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

#[tokio::test]
#[ignore = "launched by the frontend integration suite, never by ordinary cargo test"]
async fn browser_fixture_server() {
    let path =
        PathBuf::from(std::env::var_os("PATHNOTE_BROWSER_FIXTURE").expect("fixture path required"));
    assert!(
        path.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(std::env::temp_dir().canonicalize().unwrap()),
        "fixture must be in a private temporary directory"
    );
    let stop = path.with_extension("stop");
    let app = LearningApp::spawn_browser().await;
    let other = users::Entity::find()
        .filter(users::Column::Email.eq("other@example.test"))
        .one(app.db.as_ref())
        .await
        .unwrap()
        .unwrap();
    let fixture = json!({
        "baseURL":"http://127.0.0.1:4180",
        "owner":{"access_token":app.owner_token,"token_type":"Bearer","expires_in":3600,"user":{"id":app.owner_id,"name":"owner","email":"owner@example.test","avatar_url":null}},
        "other":{"access_token":app.other_token,"token_type":"Bearer","expires_in":3600,"user":{"id":other.id,"name":"other","email":"other@example.test","avatar_url":null}}
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(serde_json::to_string(&fixture).unwrap().as_bytes())
        .unwrap();
    file.sync_all().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    while !stop.exists() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.cleanup().await;
    std::fs::remove_file(path).unwrap();
}
