use server::response::ApiResponse;
#[test]
fn success_message_is_optional() {
    assert_eq!(
        serde_json::to_value(ApiResponse::ok(7)).unwrap(),
        serde_json::json!({"data":7})
    );
    assert_eq!(
        serde_json::to_value(ApiResponse::ok(7).with_message("Đã lưu.")).unwrap(),
        serde_json::json!({"data":7,"message":"Đã lưu."})
    );
}
