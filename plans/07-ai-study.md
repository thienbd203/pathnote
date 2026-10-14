# Học cùng AI

## Dùng trên frontend local

Gõ `/` → **Học cùng AI**. Nhập điều muốn học thêm → **Sao chép prompt và bài học** →
gửi ChatGPT → dán JSON → **Xem trước** → **Thêm vào cuối bài**.
Nếu ghi chú trống, cần nhập chủ đề/yêu cầu trước khi sao chép; prompt nói rõ không
có bài gốc. Không gọi API ChatGPT hoặc yêu cầu API key.

Prompt đọc toàn tài liệu editor, gồm tiêu đề/chủ đề trong ghi chép, text, bảng từ,
IPA/nghĩa/ví dụ và đề bài tương tác được lấy qua API public. Không xuất cookie/JWT,
ID nội bộ, URL media private hay đáp án từ API definition/lượt làm. Tệp chỉ có
placeholder vì nội dung file không thuộc JSON editor. Không OCR/tự đọc tệp.

JSON hỗ trợ heading, paragraph, list, vocabulary và exercise. Dùng lại parser/trim
hiện có; bảy dạng bài tập. Bảng từ bổ sung là bảng mới trong section, không tự sửa
hoặc merge vào bảng cũ. Prompt yêu cầu tránh lặp từ không cần thiết. Đề/đáp án được
lưu riêng, mỗi câu một node. Nội dung cũ được giữ; section dùng các node thông
thường dưới tiêu đề Học cùng AI, không tạo một editor riêng.

## Hai bước lưu, không ghi đè Yjs

1. Client tạo key UUID và payload một lần: document section, placeholder IDs 1..N,
   definitions mỗi câu; giữ nguyên payload qua retry (kể cả UUID của từ vựng).
2. POST `/api/v1/sessions/{id}/ai-study/imports`: typed document/question validation
   và chuẩn bị hash/paths ngoài khóa. Trong transaction lấy parent lock, kiểm tra
   lại quyền, tạo câu hỏi và ai_imports receipt. Tất cả resource của batch cùng commit.
3. Retry cùng key/body của cùng user và bài trả cùng IDs/content. Key với body khác
   trả 409; key của user/bài khác trả 404. Chưa dùng được không có quyền note/cha xóa.
   Request và document tối đa 256 KiB, tổng tối đa 100 câu, slots phải đủ và duy nhất;
   không nhận media refs hoặc marker import khác trong payload mới.
4. Receipt trả section không có đáp án và heading attrs.aiImportId. Editor kiểm tra
   marker hiện có để không chèn trùng cùng operation, validate lại với bài hiện tại,
   rồi append bằng một transaction editor. Phần collaborator sửa trong lúc chuẩn bị
   vẫn ở đó. Dùng autosave hoặc Yjs/Hocuspocus hiện có để lưu, không PUT JSON ghi đè
   tài liệu realtime. Ctrl+Z dùng lịch sử editor như node thường.
5. Lưu note thành công đánh dấu receipt attached cùng transaction với note và kho từ.
   Conflict/validation rollback không đánh dấu attached. Có reference tới câu hỏi
   mà marker bị xóa vẫn được đánh dấu để cleanup không xóa nhầm tài nguyên đang dùng.

Hai bước này không phải transaction chung giữa browser, WebSocket và PostgreSQL.
UI vẫn dùng trạng thái lưu ghi chú để biết đã lưu thật. Retry/import cache giải
quyết mất response; không tuyên bố dữ liệu đã durable ngay khi section hiện ở editor.
Một key là một thao tác, không phải chống người dùng cố ý nhập lại cùng JSON bằng
một thao tác mới. Cache retry trong dialog không lưu qua F5; draft chưa được server
xác nhận có thể mất khi người dùng rời trang theo cảnh báo hiện có.

## Cleanup

Server quét mỗi giờ, tối đa 100 receipt chưa attached mỗi lượt, cũ hơn 24 giờ kể từ
lần prepare gần nhất. Retry pending gia hạn 24 giờ và giữ cùng IDs. Cleanup và lưu
note lấy cùng parent lock, đọc lại attached/created_at sau khi chờ; delete receipt
và các câu hỏi chưa từng dùng trong cùng transaction. Chỉ query metadata/IDs,
không tải/duyệt toàn document khi giữ khóa.

Receipt đã từng attached và câu hỏi của nó được giữ kể cả khi xóa section để
Ctrl+Z/redo không hỏng. Đây là retention có chủ đích, chưa có chính sách hard-delete
bài đã dùng. Không thêm queue framework; cleanup dùng interval Tokio hiện có,
nghiệp vụ transaction dùng SeaORM, UUID/hash dùng thư viện đã có.

## Đọc code

- `server/src/modules/ai_study/service.rs`: prepare, fingerprint/retry, attach.
- `cleanup.rs`: thu hồi batch chưa từng dùng; `mod.rs`: endpoint Bearer JWT.
- `migration/src/m20261014_030001_create_ai_imports.rs`: receipt JSONB, FK/index.
- `notes/document`: Reference::AIStudy và heading attrs typed UUID; reference kiểm
  tra đúng bài. REST/Yjs đánh dấu attached sau revision check, trước commit.
- Web `lib/ai-study.ts` và `AIStudyDialog.vue`: copy context, parser/preview,
  stable import payload, marker check và append. AI vẫn là dialog trong slash menu.
- Hocuspocus chỉ thêm heading attribute cho mode Rust; schema Laravel giữ nguyên.

## Kiểm tra

72 API test thông thường qua; test hai client Hocuspocus thật chạy riêng qua:
marker đồng bộ và giữ qua restart, không nhân đôi. Retry cùng body trả cùng
content/IDs; đổi key payload bị chặn; note conflict rollback attached; cleanup
không xóa resource đã lưu/undo. 159 test frontend, 10 collaboration test qua;
typecheck/build và Clippy qua. Có tối ưu query metadata sau regression; targeted
AI test chạy lại qua. Chưa thử UI với Google thật trong đợt này.

Migration đã áp vào PostgreSQL local. Chưa deploy production, chưa commit.

```sh
cargo run -p migration -- up
cargo test --workspace
NODE_BIN=/opt/homebrew/opt/node@22/bin/node cargo test -p server --test api collaboration::hocuspocus_two_clients_save_to_rust_and_reload_from_postgres -- --ignored
```
