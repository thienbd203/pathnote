use super::learning_helpers::LearningApp;
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::future::IntoFuture;
fn entry(word: &str, ipa: &str) -> Value {
    json!({"word":word,"ipa":ipa,"topics":[],"senses":[{"partOfSpeech":"noun","meaning":"meaning","examples":[{"text":"Example"}]}]})
}
fn pairs() -> Value {
    json!({"version":1,"kind":"ipa-pairs","locale":"en-GB","sounds":["iː","ɪ"],"pairs":[{"left":entry("sheep","/ʃiːp/"),"right":entry("ship","/ʃɪp/")} ]})
}
#[tokio::test]
async fn ipa_words_are_private_locale_scoped_paginated_trimmed_and_duplicate_import_is_idempotent()
{
    let app = LearningApp::spawn().await;
    let auth = format!("Bearer {}", app.owner_token);
    let body = json!({"version":1,"locale":"en-US","entries":(0..131).map(|i|entry(&format!("word {i}"),"")).collect::<Vec<_>>()});
    app.api
        .get("/api/v1/ipa-vocabulary?locale=en-US")
        .await
        .assert_status_unauthorized();
    let r = app
        .api
        .post("/api/v1/ipa-vocabulary/import")
        .add_header("authorization", &auth)
        .json(&body)
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["added"], 131);
    let r = app
        .api
        .get("/api/v1/ipa-vocabulary?locale=en-US")
        .add_header("authorization", &auth)
        .await;
    r.assert_status_ok();
    assert_eq!(
        r.json::<Value>()["data"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
    assert_eq!(
        r.json::<Value>()["meta"],
        json!({"current_page":1,"last_page":2,"per_page":100,"total":131})
    );
    let r = app
        .api
        .get("/api/v1/ipa-vocabulary?locale=en-US&page=2")
        .add_header("authorization", &auth)
        .await;
    assert_eq!(
        r.json::<Value>()["data"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        31
    );
    let preview = json!({"version":1,"locale":"en-US","entries":[entry(" WORD   130 ",""),entry("new word",""),entry("NEW WORD","")]});
    for (token, expected) in [
        (&app.owner_token, json!(["WORD   130", "NEW WORD"])),
        (&app.other_token, json!(["NEW WORD"])),
    ] {
        let r = app
            .api
            .post("/api/v1/ipa-vocabulary/duplicates")
            .add_header("authorization", format!("Bearer {token}"))
            .json(&preview)
            .await;
        r.assert_status_ok();
        assert_eq!(r.json::<Value>()["data"]["duplicates"], expected);
    }
    let unchanged = app
        .api
        .get("/api/v1/ipa-vocabulary?locale=en-US")
        .add_header("authorization", &auth)
        .await;
    assert_eq!(unchanged.json::<Value>()["meta"]["total"], 131);
    let body = json!({"version":1,"locale":"en-US","entries":[entry("  WORD   0  ",""),entry("Word 0","")]});
    let (a, b) = tokio::join!(
        app.api
            .post("/api/v1/ipa-vocabulary/import")
            .add_header("authorization", &auth)
            .json(&body)
            .into_future(),
        app.api
            .post("/api/v1/ipa-vocabulary/import")
            .add_header("authorization", &auth)
            .json(&body)
            .into_future()
    );
    a.assert_status_ok();
    b.assert_status_ok();
    assert_eq!(a.json::<Value>()["data"]["added"], 0);
    assert_eq!(
        b.json::<Value>()["data"]["skipped"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for (locale, token) in [("en-GB", &app.owner_token), ("en-US", &app.other_token)] {
        let r = app
            .api
            .get(&format!("/api/v1/ipa-vocabulary?locale={locale}"))
            .add_header("authorization", format!("Bearer {token}"))
            .await;
        r.assert_status_ok();
        assert_eq!(r.json::<Value>()["meta"]["total"], 0);
    }
    app.api
        .get("/api/v1/ipa-vocabulary?locale=en-US&page=0")
        .add_header("authorization", &auth)
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    app.cleanup().await;
}
#[tokio::test]
async fn ipa_pairs_validate_phonemes_reverse_orientation_and_reject_all_invalid_batch_atomically() {
    let app = LearningApp::spawn().await;
    let auth = format!("Bearer {}", app.owner_token);
    let body = pairs();
    let r = app
        .api
        .post("/api/v1/ipa-pairs/import")
        .add_header("authorization", &auth)
        .json(&body)
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["added"], 1);
    let mut reverse = body.clone();
    reverse["sounds"] = json!(["ɪ", "iː"]);
    reverse["pairs"][0] =
        json!({"left":body["pairs"][0]["right"],"right":body["pairs"][0]["left"]});
    let preview = app
        .api
        .post("/api/v1/ipa-pairs/duplicates")
        .add_header("authorization", &auth)
        .json(&reverse)
        .await;
    preview.assert_status_ok();
    assert_eq!(
        preview.json::<Value>()["data"]["duplicates"],
        json!(["ship ↔ sheep"])
    );
    let foreign = app
        .api
        .post("/api/v1/ipa-pairs/duplicates")
        .add_header("authorization", format!("Bearer {}", app.other_token))
        .json(&reverse)
        .await;
    foreign.assert_status_ok();
    assert_eq!(foreign.json::<Value>()["data"]["duplicates"], json!([]));
    let r = app
        .api
        .post("/api/v1/ipa-pairs/import")
        .add_header("authorization", &auth)
        .json(&reverse)
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["data"]["added"], 0);
    let r = app
        .api
        .get("/api/v1/ipa-pairs?locale=en-GB")
        .add_header("authorization", &auth)
        .await;
    r.assert_status_ok();
    assert_eq!(
        r.json::<Value>()["data"]["sets"][0]["pairs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut bad = pairs();
    bad["pairs"] = json!([{"left":entry("cheap","/tʃiːp/"),"right":entry("chip","/tʃɪp/")},{"left":entry("bad","/bæd/"),"right":entry("bed","/bed/")}]);
    app.api
        .post("/api/v1/ipa-pairs/import")
        .add_header("authorization", &auth)
        .json(&bad)
        .await
        .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let r = app
        .api
        .get("/api/v1/ipa-pairs?locale=en-GB")
        .add_header("authorization", &auth)
        .await;
    assert_eq!(r.json::<Value>()["meta"]["total"], 1);
    let us = json!({"version":1,"kind":"ipa-pairs","locale":"en-US","sounds":["e","æ"],"pairs":[{"left":entry("bed","/bɛd/"),"right":entry("bad","/bæd/")}]});
    app.api
        .post("/api/v1/ipa-pairs/import")
        .add_header("authorization", &auth)
        .json(&us)
        .await
        .assert_status_ok();
    app.cleanup().await;
}
