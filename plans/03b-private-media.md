# Ảnh và tệp đính kèm riêng tư

Mở lại upload ảnh/file trong editor hiện có, lưu trên Rust và tham chiếu bằng
mediaId trong JSONB. Không lưu base64, đường dẫn filesystem hay URL công khai
trong tài liệu. Realtime, bài tập và dịch vụ pronunciation vẫn là bước riêng.

## Thư viện và phần ứng dụng

- Axum `Multipart`/`DefaultBodyLimit` đọc và giới hạn request; không tự parse multipart.
- `image 0.25.10` nhận dạng và decode PNG/JPEG/WebP; giới hạn kích thước và bộ nhớ.
- `infer 0.19.0` nhận dạng PDF/Office cũ từ bytes.
- `zip 8.6.0` và `roxmltree 0.21.1` kiểm tra gói Office XML; không tự viết ZIP/XML parser.
- Tokio filesystem xử lý I/O; SeaORM migration/entity quản lý metadata.
- Code riêng của Pathnote chỉ chọn định dạng/giới hạn, kiểm tra quyền note,
  phối hợp metadata với file và kiểm tra media reference trong cây tài liệu.

Đọc `modules/media` theo mod.rs → handlers.rs → validation.rs → service.rs.
Handler nhận Multipart cuối cùng vì nó tiêu thụ body. `Field` phải được drop
trước khi đọc field kế tiếp: đây là ví dụ thực tế về vòng đời tài nguyên trong Rust.
Decode ảnh chạy qua `spawn_blocking`, không chặn runtime async bằng CPU work.

## Contract API

```text
POST /api/v1/sessions/{id}/images                 multipart field image
GET  /api/v1/sessions/{id}/images/{media_id}       bytes
POST /api/v1/sessions/{id}/files                  multipart field file
GET  /api/v1/sessions/{id}/files/{media_id}        metadata
GET  /api/v1/sessions/{id}/files/{media_id}/download bytes
```

Upload trả 201 `{data:{id,name,mime_type,size}}`. JWT Bearer xác định người gọi;
owner và collaborator được upload/read. Thu hồi quyền hoặc xóa session/cha
chặn request tiếp theo bằng 404. Thiếu JWT trả 401.

- Ảnh PNG/JPEG/WebP tối đa 5 MiB, mỗi chiều tối đa 8192 px; decode có thêm
  giới hạn cấp phát 128 MiB. MIME được nhận từ bytes, không tin header client.
- PDF, DOC/DOCX, XLS/XLSX, PPT/PPTX và TXT tối đa 10 MiB. Phần mở rộng phải
  phù hợp nội dung. TXT là UTF-8, không chứa control ngoài tab/newline.
- OOXML phải có main part và Content_Types tương ứng. Content_Types tối đa
  256 KiB, parser không cho DTD; không giải nén archive ra filesystem.
- Tài liệu được download dạng attachment, không render HTML/Office inline.
- Binary có no-store, nosniff và Vary Authorization; frontend dùng blob URL.

## Storage và lifecycle

`MEDIA_ROOT` mặc định `.storage/media` (được Git bỏ qua), nằm ngoài public root.
Production dùng đường dẫn tuyệt đối ngoài release, ví dụ
`/var/lib/pathnote/media`, và backup cùng database. File có UUID làm storage key,
không dùng tên do client gửi làm đường dẫn. Tên hiển thị loại control/path separator;
Content-Disposition dùng filename UTF-8 theo RFC 5987. Trên Unix thư mục mới 0700,
file mới 0600. Process chạy Rust cần quyền ghi vào MEDIA_ROOT.

Validate bytes và ghi file + fsync chạy trước transaction. Transaction chỉ khóa cha,
kiểm tra lại quyền và insert metadata. Upload kiểm tra quyền hai lần: trước đọc
multipart và sau khi lấy khóa. Nếu DB thất bại, xóa file khi đã xác nhận không có
metadata; nếu kết quả commit chưa rõ thì giữ file để tránh mất dữ liệu.

Server quét mỗi giờ, xóa file UUID chưa có metadata và cũ ít nhất 24 giờ.
Upload giữ khóa file của hệ điều hành đến khi ghi DB/compensation xong; cleanup
bỏ qua file đang khóa, kể cả upload kéo dài. Cleanup không xóa media có record,
file mới, symlink hoặc tên không phải UUID. Filesystem cần hỗ trợ khóa file.
Có thể chạy thủ công: `cargo run -p server --bin media-cleanup`.
Crash vẫn có thể để lại orphan tạm thời trước lượt quét kế tiếp.

Xóa node không xóa file ngay vì undo/redo và draft ở tab khác có thể cần nó.
Xóa mềm session chặn API nhưng giữ bytes. Chưa có endpoint hard-delete media.

## Tiptap và frontend

Node image/file có attrs typed bằng Serde. Validate và duyệt cây thu thập media
references trước transaction; query theo session và IDs bên trong transaction lưu note. Media của buổi khác
hoặc khác loại bị trả 422, kể cả người gọi sở hữu cả hai buổi.

Frontend dùng lại picker, progress, retry/cancel, resize/caption và file node.
Adapter chuyển sang `/api/v1`; slash và paste/drop ảnh/file đã mở lại. Các block
vocabulary và IPA/dịch đã mở ở bước 4–5; exercise còn chờ API Rust.

## Kiểm tra

`server/tests/api/media.rs` kiểm tra upload/download, bytes MIME, spoofed types,
size/dimension, multipart lạ, TXT/OOXML, filename, storage lỗi, note refs và thu hồi
chia sẻ. Database và storage test đều dùng phần riêng được dọn sau test thành công.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p migration -- up
```

Tham khảo:
[Axum Multipart](https://docs.rs/axum/latest/axum/extract/struct.Multipart.html),
[image ImageReader](https://docs.rs/image/0.25.10/image/struct.ImageReader.html).
