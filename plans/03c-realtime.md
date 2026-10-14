# Realtime: giữ Hocuspocus, nối với Rust

## Phạm vi và phần thư viện làm sẵn

Giữ Hocuspocus + Yjs cho giao thức đồng bộ, phòng tài liệu và awareness.
Rust không viết lại CRDT hay WebSocket server. Mã ứng dụng chỉ nối quyền bài học,
validation JSON/media và persistence qua HTTP.

- Frontend lấy cấu hình từ `GET /api/v1/sessions/{id}/collaboration`.
- Provider dùng JWT hiện có, lấy token mới qua interceptor refresh khi cần.
- Hocuspocus gọi `POST /api/v1/sessions/{id}/collaboration/authorize` với JWT và secret server.
- Rust kiểm tra JWT, chủ sở hữu hoặc cộng tác viên và các cấp cha chưa xóa; thiếu quyền trả 404.
- Hocuspocus đọc/ghi `GET/PUT /api/v1/internal/notes/{id}` bằng secret server.
- Migration thêm `notes.yjs_state` nullable (BYTEA). JSONB và binary state được lưu trong cùng transaction.
- Hocuspocus dùng transformer tạo Yjs từ JSON đúng một lần; các lần sau nạp binary đã lưu.
  Không tái tạo từ JSON mỗi lần mở: sẽ mất lịch sử và có thể nhân đôi nội dung.
- PUT validate Tiptap, decode base64 và chuẩn bị từ vựng trước transaction.
  Trong khóa cha, kiểm tra lại quyền, revision/schema và media thuộc bài.
  Retry cùng binary/content là idempotent. Rust tin binary do bridge Hocuspocus tạo;
  Rust không tự giải mã Yjs hay đối chiếu binary với JSON.
- HTTP autosave bị chặn khi bật realtime hoặc note đã có binary; không có fallback âm thầm
  khi mất kết nối. Frontend giữ nội dung đang viết và cảnh báo trước khi rời trang.

## Cấu hình local

Rust `.env`:

```ini
COLLABORATION_URL=ws://localhost:1235
COLLABORATION_SECRET=<secret riêng ít nhất 32 ký tự>
```

Hocuspocus `.env.rust` (Git bỏ qua, không dùng chung file môi trường Laravel):

```ini
COLLABORATION_API_URL=http://127.0.0.1:3000
COLLABORATION_API_PREFIX=/api/v1
COLLABORATION_SECRET=<cùng secret với Rust>
COLLABORATION_HOST=127.0.0.1
COLLABORATION_PORT=1235
COLLABORATION_ORIGINS=http://localhost:5173,http://127.0.0.1:5173
```

```sh
# pathnote-rust
cargo run -p migration -- up
cargo run -p server
# pathnote-collaboration, Node >=22
node --env-file=.env.rust server/start.mjs
# pathnote-web
npm run dev
```

Khởi động lại Rust sau khi đổi `.env`. Production dùng WSS, secret chỉ ở hai server,
media/key/data nằm ngoài release. Server Laravel cũ có thể giữ cổng 1234.
Không chuyển dữ liệu hoặc cấu hình production trong bước này.

## Kiểm tra đã thực hiện

- API: cấu hình không lộ secret; JWT + quyền; secret sai/thiếu bị từ chối;
  gỡ cộng tác viên; tắt bridge; snapshot idempotent; revision conflict; media sai bài;
  note hoặc cha đã xóa; không ghi đè qua HTTP autosave.
- Hai provider thật qua WebSocket → Hocuspocus → HTTP Rust → PostgreSQL schema riêng:
  sửa chung, danh tính awareness từ server, lưu, restart Hocuspocus, mở lại chỉ một paragraph,
  gỡ cộng tác viên và ngắt quyền kết nối.
- Frontend typecheck/build và test suite; Hocuspocus test suite; Rust Clippy.

```sh
cargo test --workspace
# Cần Node >=22 và npm ci trong pathnote-collaboration, chạy riêng bài test cross-stack:
cargo test -p server --test api collaboration::hocuspocus_two_clients_save_to_rust_and_reload_from_postgres -- --ignored
```

## Giới hạn và phần còn lại

- Chưa xác nhận thao tác hai người qua UI với Google thật. API test dùng tài khoản test riêng.
- Quyền và hạn JWT kiểm tra lại tối đa mỗi 5 giây; không phải thu hồi ngay tức thì.
  Khi hết hạn provider lấy JWT mới qua luồng refresh/reconnect. Logout frontend đóng kết nối;
  JWT đã cấp vẫn hợp lệ tối đa 15 phút như mô hình auth đã chốt.
- Chạy một Hocuspocus instance cho dữ liệu này. Chưa bật Redis coordination cho nhiều instance.
- Kho từ vựng, flashcard và IPA/dịch đã nối ở bước 4–5; bảy dạng bài tập đã nối ở bước 6a–6b; Học cùng AI đã nối ở [bước 7](07-ai-study.md).
- Bridge đã commit local trong lượt triển khai trước; chưa xác nhận deploy production.

Nguồn thư viện:
[Database extension](https://tiptap.dev/docs/hocuspocus/server/extensions/database),
[Hooks](https://tiptap.dev/docs/hocuspocus/server/hooks).
