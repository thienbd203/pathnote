use super::{learning_helpers::LearningApp, media::lesson};
use axum::http::StatusCode;
use serde_json::{json, Value};
fn question(kind: &str, options: Value, answers: Value) -> Value {
    json!({"type":kind,"prompt":"Practice","options":options,"answers":answers,"explanation":"Review","caseSensitive":false})
}
async fn solve(app: &LearningApp, q: Value, answer: Value, correct: bool) {
    let s = lesson(app).await;
    let auth = format!("Bearer {}", app.owner_token);
    let r = app
        .api
        .post(&format!("/api/v1/sessions/{s}/exercises"))
        .add_header("authorization", &auth)
        .json(&json!({"title":"Practice","questions":[q]}))
        .await;
    r.assert_status(StatusCode::CREATED);
    let id = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let base = format!("/api/v1/sessions/{s}/exercises/{id}");
    let r = app.api.get(&base).add_header("authorization", &auth).await;
    r.assert_status_ok();
    assert!(r.json::<Value>()["data"]["questions"][0]
        .get("answers")
        .is_none());
    let r = app
        .api
        .post(&format!("{base}/attempts"))
        .add_header("authorization", &auth)
        .json(&json!({}))
        .await;
    r.assert_status_ok();
    let aid = r.json::<Value>()["data"]["id"].as_i64().unwrap();
    let r = app
        .api
        .post(&format!("{base}/attempts/{aid}/answers"))
        .add_header("authorization", &auth)
        .json(&json!({"position":0,"answer":answer,"try":0}))
        .await;
    r.assert_status_ok();
    let result = &r.json::<Value>()["data"]["results"][0];
    assert_eq!(result["correct"], correct);
    assert_eq!(result["final"], correct);
    assert_eq!(result.get("answers").is_some(), correct);
}
#[tokio::test]
async fn all_additional_exercise_types_grade_through_http_and_preserve_trim_case_and_alternatives()
{
    let app = LearningApp::spawn().await;
    solve(
        &app,
        question("classify", json!(["Common", "Proper"]), json!(["Proper"])),
        json!("Proper"),
        true,
    )
    .await;
    let mut matching = question(
        "matching",
        json!(["doctor", "teacher"]),
        json!(["teacher", "doctor"]),
    );
    matching["items"] = json!(["giáo viên", "bác sĩ"]);
    solve(
        &app,
        matching.clone(),
        json!("[\"teacher\",\"doctor\"]"),
        true,
    )
    .await;
    solve(&app, matching, json!("[\"doctor\",\"teacher\"]"), false).await;
    // The word bank is a multiset: two equal words must both be used.
    let ordering = question(
        "ordering",
        json!(["a", "teacher", "is", "a", "teacher."]),
        json!(["a teacher is a teacher."]),
    );
    solve(
        &app,
        ordering.clone(),
        json!("a   teacher is a teacher."),
        true,
    )
    .await;
    solve(&app, ordering, json!("teacher is a a teacher."), false).await;
    let translation = question(
        "translation",
        json!([]),
        json!(["I am a teacher.", "I'm a teacher."]),
    );
    solve(&app, translation, json!("  I'M   A TEACHER.  "), true).await;
    let mut correction = question("correction", json!([]), json!(["She is a doctor."]));
    correction["caseSensitive"] = json!(true);
    solve(&app, correction.clone(), json!("she is a doctor."), false).await;
    solve(&app, correction, json!("She is a doctor."), true).await;
    app.cleanup().await;
}
#[tokio::test]
async fn translation_accepts_optional_final_period_without_ignoring_content_or_case_rules() {
    let app = LearningApp::spawn().await;
    let translation = question(
        "translation",
        json!([]),
        json!(["This is Anna's phone number.", "It is Anna's phone number."]),
    );
    for answer in [
        "This is Anna's phone number",
        "  IT   IS Anna's phone number.  ",
    ] {
        solve(&app, translation.clone(), json!(answer), true).await;
    }
    for answer in [
        "This is Annas phone number",
        "This is Anna's phone",
        "This is Anna's phone number?",
    ] {
        solve(&app, translation.clone(), json!(answer), false).await;
    }
    let mut sensitive = translation;
    sensitive["caseSensitive"] = json!(true);
    solve(
        &app,
        sensitive.clone(),
        json!("This is Anna's phone number"),
        true,
    )
    .await;
    solve(&app, sensitive, json!("this is Anna's phone number"), false).await;
    solve(
        &app,
        question(
            "translation",
            json!([]),
            json!(["This is Anna's phone number"]),
        ),
        json!("This is Anna's phone number."),
        true,
    )
    .await;
    for kind in ["correction", "fill"] {
        solve(
            &app,
            question(kind, json!([]), json!(["This is Anna's phone number."])),
            json!("This is Anna's phone number"),
            false,
        )
        .await;
    }
    app.cleanup().await;
}
#[tokio::test]
async fn rejects_invalid_type_definitions_and_malformed_matching_or_invented_word_answers() {
    let app = LearningApp::spawn().await;
    let s = lesson(&app).await;
    let auth = format!("Bearer {}", app.owner_token);
    let base = format!("/api/v1/sessions/{s}/exercises");
    let mut bad_matching = question("matching", json!(["a", "b"]), json!(["a", "a"]));
    bad_matching["items"] = json!(["one", "two"]);
    let mut extra_items = question("fill", json!([]), json!(["as"]));
    extra_items["items"] = json!(["one", "two"]);
    for q in [
        bad_matching,
        extra_items,
        question("classify", json!(["same", "same"]), json!(["same"])),
        question("translation", json!(["choice"]), json!(["choice"])),
        question("ordering", json!(["a", "a", "b"]), json!(["a b b"])),
    ] {
        app.api
            .post(&base)
            .add_header("authorization", &auth)
            .json(&json!({"title":"Practice","questions":[q]}))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut matching = question("matching", json!(["a", "b"]), json!(["b", "a"]));
    matching["items"] = json!(["one", "two"]);
    for (q, bad_answers) in [
        (
            matching,
            vec![
                json!("bad JSON"),
                json!("[\"a\"]"),
                json!("[\"a\",\"a\"]"),
                json!("[\"a\",\"outside\"]"),
            ],
        ),
        (
            question(
                "ordering",
                json!(["I", "am", "a", "teacher."]),
                json!(["I am a teacher."]),
            ),
            vec![json!("I am a doctor."), json!("I am a a teacher.")],
        ),
    ] {
        let r = app
            .api
            .post(&base)
            .add_header("authorization", &auth)
            .json(&json!({"title":"Practice","questions":[q]}))
            .await;
        r.assert_status(StatusCode::CREATED);
        let id = r.json::<Value>()["data"]["id"].as_i64().unwrap();
        let path = format!("{base}/{id}");
        let r = app
            .api
            .post(&format!("{path}/attempts"))
            .add_header("authorization", &auth)
            .json(&json!({}))
            .await;
        let aid = r.json::<Value>()["data"]["id"].as_i64().unwrap();
        for answer in bad_answers {
            app.api
                .post(&format!("{path}/attempts/{aid}/answers"))
                .add_header("authorization", &auth)
                .json(&json!({"position":0,"answer":answer,"try":0}))
                .await
                .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        }
        let r = app.api.get(&path).add_header("authorization", &auth).await;
        assert!(r.json::<Value>()["data"]["attempt"]["results"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    app.cleanup().await;
}
#[tokio::test]
async fn matching_accepts_eight_long_pairs_within_its_own_answer_limit() {
    let app = LearningApp::spawn().await;
    let options: Vec<String> = (0..8).map(|i| format!("{i}{}", "x".repeat(199))).collect();
    let mut q = question("matching", json!(options), json!(options));
    q["items"] = json!((0..8).map(|i| i.to_string()).collect::<Vec<_>>());
    let answer = serde_json::to_string(&options).unwrap();
    assert!(answer.len() > 1000);
    solve(&app, q, json!(answer), true).await;
    app.cleanup().await;
}
