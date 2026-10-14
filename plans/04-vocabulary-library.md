# Kho từ vựng từ ghi chú

## Phạm vi hiện tại

Lấy từ trong node `vocabulary`, không đoán từ trong văn bản thường.
Một `vocabularyEntry` có nhiều `vocabularySense`; mỗi nghĩa là một thẻ.
Lưu từ, IPA, giọng US/UK, loại từ, nghĩa, chủ đề, ví dụ và nguồn buổi học.
Không sửa JSON gốc khi chuẩn hóa dữ liệu trong kho.

Frontend mở lại bảng từ vựng: nhập tay, nhập JSON, merge giữ ID, chỉnh từ/nghĩa,
ví dụ/chủ đề và đọc bằng giọng của trình duyệt. Mục **Kho từ vựng** dùng API Rust.
Trong menu buổi học có **Từ vựng của buổi học**. Flashcard và IPA/dịch tự động đã nối ở [bước 5](05-vocabulary-reviews.md). Không gửi JWT Rust tới endpoint Laravel cũ.

## Quyết định về queue

Rust có thư viện job queue, ví dụ [Apalis](https://apalis.dev/), với backend
[PostgreSQL](https://docs.rs/apalis-postgres/latest/apalis_postgres/).
Không có hệ thống job/worker mặc định trong Axum.

Bước này không dùng queue: công việc chỉ duyệt tài liệu bị giới hạn kích thước,
đọc thẻ đã có và ghi phần thay đổi trong PostgreSQL, không gọi dịch vụ bên ngoài.
Chạy trong transaction lưu note để ghi chú và kho từ cùng thành công hoặc cùng rollback.
Sau khi lưu trả thành công, không có trạng thái chờ worker hay cửa sổ dữ liệu cũ.
Không dùng `tokio::spawn` như một queue bền vững: task trong RAM có thể mất khi restart.
Nếu sau này có enrichment nặng hoặc đo thấy lưu note chậm, đánh giá thư viện queue
và khả năng enqueue nguyên tử với transaction trước khi chuyển cơ chế.
Chưa có benchmark tải production để kết luận throughput của cách hiện tại.

## Luồng dữ liệu và các phần cần đọc

1. `notes/document/vocabulary.rs` deserialize sang cây enum đã định nghĩa, dùng
   pattern matching và đệ quy để lấy table → entry → sense → example.
2. `vocabulary/sync.rs` nhận connection transaction từ bước lưu ghi chú.
3. Khóa `(session_id, source_sense_id)` giữ ID thẻ khi sửa hoặc undo.
   Chỉ upsert thẻ có nội dung thay đổi, theo lô 100; số 100 là kích thước lô SQL,
   không phải giới hạn số từ trong một bảng.
4. Thẻ mất khỏi ghi chú chuyển `active=false`, không xóa ID; undo kích hoạt lại.
5. Từ/nghĩa/IPA/ví dụ trim đầu cuối; chủ đề trim, loại trùng và sort.
   `word_key` là SHA-256 của từ lower-case và gom whitespace, dùng để nhóm từ.
6. Cả HTTP autosave lẫn snapshot Hocuspocus gọi cùng hàm sync trước commit.
7. `GET /api/v1/vocabulary` dùng lại scope quyền note, tự ẩn dữ liệu nếu thu hồi
   chia sẻ hoặc xóa mềm session/course/place; không cần chạy job để thu hồi quyền.

Cột/bảng nghiệp vụ được tạo bằng SeaQuery migration.
Không tạo hạ tầng queue hoặc migration của package khi chưa sử dụng package đó.

## API

`GET /api/v1/vocabulary`, yêu cầu Bearer JWT.

- `session_id`: tùy chọn, không có quyền mở bài thì trả 404.
- `topic`: tối đa 60 ký tự, JSONB containment.
- `q`: tối đa 200 ký tự, tìm từ hoặc nghĩa; `%` và `_` là ký tự thường.
- `page`: từ 1, mỗi trang 50 thẻ, sort word rồi ID.

Response có `data` và `meta`: số thẻ, số từ khác nhau, số nghĩa không rỗng,
chủ đề, nguồn bài đang lọc và phân trang. `syncing=false` vì đồng bộ cùng transaction.
Progress hiện đọc theo từng người và study_key ở bước 5; không dùng trạng thái chung của thẻ.

## Migration và backfill local

```sh
cargo run -p migration -- up
cargo run -p server --bin vocabulary-sync
```

Lệnh thứ hai duyệt ghi chú cũ theo trang 100, mỗi bài một transaction và khóa cha,
không sửa revision/nội dung ghi chú. Chạy lại không nhân đôi thẻ. Ghi chú mới tự đồng bộ
khi lưu, không phải chạy cron. Đã áp migration và backfill trên database local.

## Kiểm tra

- Bảng 131 từ, phân trang 50/50/31, lọc chủ đề và literal wildcard.
- Trim và chuẩn hóa khóa từ; nhiều nghĩa, word_count khác total.
- Cập nhật giữ ID; lưu không đổi không ghi lại thẻ; xóa/undo không sinh ID mới.
- Conflict note không thay kho; collaborator nhìn được, bị gỡ thì mất quyền ngay ở API.
- Xóa session ẩn từ; JWT thiếu và filter không hợp lệ bị từ chối.
- Hocuspocus snapshot và backfill idempotent. Hai provider thật đã thử thêm bảng Yjs,
  lưu qua Rust rồi kiểm tra từ/nghĩa xuất hiện trong kho và khôi phục sau restart.
- Frontend tests/build và Rust Clippy.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Chưa thử UI với tài khoản Google thật trong bước này. Không deploy hoặc commit tự động.
