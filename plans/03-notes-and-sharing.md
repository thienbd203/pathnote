# Giai đoạn 3 — Ghi chú Tiptap và chia sẻ

Đã triển khai lưu JSONB, revision chống ghi đè, chia sẻ theo tài khoản Google
và danh sách được chia sẻ. Frontend dùng lại trang SessionView, editor và
autosave hiện có. Realtime đã nối ở [bước 3c](03c-realtime.md).

## Đọc code để học Rust

1. `migration/src/m20261013_090001_create_notes.rs`: SeaQuery tạo `notes`
   (khóa chính session_id, content JSONB) và `note_collaborators`.
2. `server/src/entities/note_collaborators.rs`: hai FK và khóa chính kép.
   `sessions.rs`/`users.rs` có `Related` với `via()` qua bảng nối; service dùng
   `session.find_related(users::Entity)` để lấy cộng tác viên.
3. `server/src/modules/notes/document/node.rs`: enum `Node` đệ quy qua
   `Vec<Node>`, có `#[serde(tag = "type")]`. JSON paragraph và heading trở
   thành hai variant khác nhau; một trường level kiểu chuỗi bị Serde từ chối.
4. `document/attributes.rs` và `marks.rs`: kiểu thuộc tính, enum locale,
   part of speech, dấu định dạng và `deny_unknown_fields`.
5. `document/validation.rs`: duyệt cây bằng match và đệ quy; kiểm tra quan hệ
   cha/con, giới hạn, link và bảng. Đây là luật của ứng dụng, không tự viết
   JSON parser, ORM, hay framework validate thay cho thư viện.
6. `dto.rs`: request, response và `SaveOutcome::Saved/Conflict`.
7. `service.rs`: quyền owner/collaborator, transaction và revision.
8. `handlers.rs`: ánh xạ outcome sang HTTP; `mod.rs` chỉ khai báo route.
9. `server/tests/api/notes.rs`: test HTTP theo luồng, không unit test từng helper.

Serde thay phần kiểm tra tên node, field, kiểu dữ liệu và giá trị enum. Các
ràng buộc như heading 1..3, 256 KiB, 10.000 node, sâu tối đa 32 cấp,
rowspan/colspan không chồng lấn và link an toàn vẫn cần luật kiểm tra riêng.
Không phải tất cả rule PHP có thể biến thành định nghĩa kiểu.

## Lưu tài liệu

JSON gốc được kiểm tra bằng các kiểu Rust rồi lưu nguyên dạng vào JSONB. Không
serialize lại các struct có default để tránh thêm field/null vào tài liệu của
Tiptap. PostgreSQL JSONB giữ cấu trúc và giá trị; không cam kết giữ thứ tự key.

```http
GET /api/v1/sessions/1/note
PUT /api/v1/sessions/1/note
Authorization: Bearer <access token>
```

```json
{
  "content": {"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Jobs"}]}]},
  "schema_version": 3,
  "base_revision": 0
}
```

Response `{data:{session_id,content,schema_version,revision,saved_at}}`.
Note chưa lưu trả document paragraph trống, schema 3, revision 0, saved_at null.
Lưu thành công tăng revision. Revision cũ trả 409 kèm `data` là note hiện tại;
frontend giữ draft và cho người dùng xem/giải quyết xung đột.

Schema 1..3 được nhận để kiểm tra cấu trúc; note mới khởi tạo schema 3, không
cho hạ phiên bản đã tải. Document hỗ trợ văn bản, heading, danh sách/checklist,
blockquote, code, bảng, marks và dữ liệu vocabulary/pronunciation. Ảnh và file đã được mở trong [bước private media](03b-private-media.md), có
kiểm tra reference đúng buổi học. Exercise đã được nhận ở [bước 6a](06a-inline-exercises.md): chỉ lưu ID,
kiểm tra resource thuộc bài; đáp án nằm trong DB.

Frontend mở rich text, checklist, code, bảng và định dạng chữ trong editor cũ.
Menu upload ảnh/file, bảng từ vựng và IPA/dịch tự động đã mở ở các bước 3b–5.
Bài tập bảy dạng đã mở ở bước 6a; Học cùng AI đã nối ở [bước 7](07-ai-study.md). Không fallback sang Laravel bằng JWT Rust.

## Quyền và chia sẻ

- Owner đọc/sửa note và quản lý cộng tác viên.
- Collaborator đọc/sửa note, không sửa lịch hoặc quản lý quyền chia sẻ.
- Không có quyền, session/cha đã xóa: 404; JWT thiếu/hỏng: 401.
- Mời theo email trim/lowercase của tài khoản đã đăng nhập Google; không gửi email.
- Không mời chính owner. Khóa chính kép làm lời mời lặp không tạo hai hàng.
- Thu hồi idempotent 204; request đọc/lưu tiếp theo của người bị thu hồi trả 404.
- Save, invite, revoke và xóa cha dùng cùng thứ tự khóa place, kiểm tra lại quyền
  sau khi lấy khóa. Hai request cùng revision chỉ một request lưu thành công.

```text
GET    /api/v1/sessions/{id}/collaborators
POST   /api/v1/sessions/{id}/collaborators       {"email":"friend@example.com"}
DELETE /api/v1/sessions/{id}/collaborators/{user_id}
GET    /api/v1/shared-notes?page=1&per_page=25
GET    /api/v1/shared-notes/{id}
```

Danh sách shared trả `{data,meta}` như các list Rust khác và chỉ lấy note
được mời, có session/cha hoạt động. Metadata note được tải bằng một query theo
các ID của trang, không một query cho mỗi note.

## Kiểm tra và chạy local

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p migration -- up
```

Test PostgreSQL dùng schema riêng: JSONB roundtrip, rich nodes, sai kiểu,
unknown field, nesting/size/node limits, link/bảng, conflict, save đồng thời,
invite lặp, quyền note so với lịch, thu hồi và session đã xóa.
Migration local đã áp dụng; backend đang chạy cần dùng code mới để có route notes.
Google login thật và hai tài khoản trên trình duyệt cần thử riêng.

Tham khảo:
[Serde internally tagged enum](https://serde.rs/enum-representations.html),
[SeaORM many-to-many](https://www.sea-ql.org/SeaORM/docs/1.1.x/relation/many-to-many/).
