# Pathnote Rust

Axum, SeaORM và PostgreSQL. Google login dùng `openidconnect`; phiên dùng
`axum-login`, `tower-sessions` và `tower-sessions-sqlx-store`. JWT dùng
`jsonwebtoken`, ký RS256 bằng khóa riêng của Pathnote.

Danh sách giai đoạn và checklist triển khai nằm trong [plans](plans/README.md).

## Bước học Places

Module đầu tiên của giai đoạn 2 đã có API dưới `/api/v1/places`: GET/POST cho
collection và GET/PATCH/DELETE cho một ID. JWT xác định chủ sở hữu; dữ liệu của
user khác hoặc đã xóa trả 404. Có page/per_page, tìm tên bằng q, xóa mềm và
PATCH phân biệt missing/null. Courses và Sessions đã có CRUD; sinh lịch tự động đã loại khỏi phạm vi.

Đọc [Học Rust qua Places](plans/02a-places.md) để đi từ SeaQuery migration,
entity đến extractor, handler, DTO và service. Khi muốn áp schema vào app local,
chạy `cargo run -p migration -- up` như hướng dẫn bên dưới.

Test Places dùng `TEST_DATABASE_URL` nếu được cấu hình; nếu không, dùng
DATABASE_URL local để tạo schema `pathnote_learning_test_*` riêng từng test và
xóa sau khi test thành công. Không sửa bảng trong schema public. Database
test cần quyền tạo schema. Các test auth trước đó vẫn dùng SQLite riêng.

## Bước học Courses

Module Courses có GET/POST `/api/v1/places/{place_id}/courses` và
GET/PATCH/DELETE `/api/v1/courses/{id}`. Dùng JOIN để giới hạn quyền qua place,
transaction/row lock để phối hợp tạo course với xóa place. Xem
[Courses, relation và transaction](plans/02b-courses.md).

## Bước học Sessions

GET/POST `/api/v1/courses/{id}/sessions` cho buổi học thuộc khóa;
GET/POST `/api/v1/places/{id}/sessions` chỉ cho buổi học độc lập.
GET/PATCH/DELETE `/api/v1/sessions/{id}` cho từng buổi học.
List có `page`, `per_page`, `q` tìm theo tên, sắp theo `starts_at` rồi ID.

Thời điểm gửi RFC3339 có offset hoặc `Z`; response trả UTC. `timezone` cần khi
tạo hoặc đổi thời gian/thời lượng để kiểm tra buổi học kết thúc cùng ngày địa
phương. Xem [Sessions, UTC và relation tùy chọn](plans/02c-sessions.md).

## Chạy local

Sao chép `.env.example` thành `.env`, điền PostgreSQL và Google Client ID/Secret.
Khai báo `GOOGLE_REDIRECT_URL` trong Authorized redirect URIs của Google Console.
Các URL local cần dùng cùng hostname (ví dụ `localhost`) để cookie hoạt động nhất quán.

Tạo khóa riêng một lần, giữ nguyên qua các lần khởi động và deploy:

```sh
mkdir -p .keys
chmod 700 .keys
(umask 077; openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 -out .keys/access-token.pem)
cargo run -p migration -- up
cargo run -p server
```

Không chạy lại lệnh tạo khóa để ghi đè khóa đang dùng. `.keys/` và `.env` được
Git bỏ qua. Khóa ký này khác với Google Client Secret và khác khóa RSA của test.
Production đặt khóa ngoài release, cấu hình đường dẫn tuyệt đối, URL HTTPS và
`COOKIE_SECURE=true`. Bảng phiên được package tạo bằng `PostgresStore::migrate()`;
SeaORM migration quản lý bảng nghiệp vụ.

## Luồng đăng nhập

1. Trình duyệt mở `GET /api/v1/auth/google` rồi Google chuyển về callback.
2. Rust xác minh Google, tạo hoặc cập nhật user, lưu phiên 7 ngày trong database
   và chuyển về frontend. Callback không đưa token vào URL.
3. Frontend gọi `POST /api/v1/auth/refresh` với `credentials: 'include'`.
   Request cần `Origin` khớp `FRONTEND_URL`. Response:

```json
{
  "data": {
    "access_token": "<JWT>",
    "token_type": "Bearer",
    "expires_in": 900,
    "user": { "id": 1, "name": "Example", "email": "user@example.com", "avatar_url": null }
  }
}
```

4. Frontend giữ token trong bộ nhớ, gọi `GET /api/v1/auth/me` với
   `Authorization: Bearer <JWT>`. Cookie phiên không đủ để gọi API này.
   Khi tải lại trang hoặc token hết hạn, lấy token mới qua refresh. Frontend
   nên gom các lần refresh đồng thời thành một request.
5. `POST /api/v1/auth/logout` dùng cookie phiên và Origin hợp lệ, trả 204.
   Frontend xóa token trong bộ nhớ. Phiên bị xóa nên không cấp token mới được;
   token đã cấp vẫn có hiệu lực đến khi hết hạn, tối đa 15 phút.

Session middleware chỉ gắn vào route Google, refresh và logout. API dùng JWT
không đọc session store; endpoint me truy vấn bảng user để trả thông tin hiện tại.
Route refresh có guard bắt buộc đăng nhập dùng `axum_login::predicate_required!`,
trả lỗi JSON 401 theo định dạng của API. Login và callback là route công khai;
logout có thể gọi khi phiên đã hết hạn để dọn trạng thái phía trình duyệt.

## Khóa công khai

`GET /api/v1/.well-known/jwks.json` cung cấp khóa công khai với `kid` và `alg=RS256`.
Token có `typ=at+jwt`, `iss=JWT_ISSUER`, `aud=JWT_AUDIENCE`, `sub` là ID user,
`iat`, `exp` và `token_use=access`. Backend kiểm tra chữ ký, thuật toán, key ID,
issuer, audience, loại token và thời hạn. Chỉ có một khóa hoạt động hiện tại;
chưa có cơ chế giữ nhiều khóa để xoay khóa không gián đoạn.

Frontend trên nhánh `feat/rust-auth-flow` đã nối auth và CRUD Places/Courses/Sessions
với Rust. Trang gốc mở hành trình học; trang buổi học dùng lại editor ghi chép.
Editor, chia sẻ và ảnh/tệp riêng tư đã nối. Realtime dùng Hocuspocus hiện có qua API Rust; bài tập bảy dạng đã nối. Xem [Ghi chú Tiptap và chia sẻ](plans/03-notes-and-sharing.md).
JWT hiện có audience `pathnote-api`: Hocuspocus gửi token tới Rust để xác minh và kiểm tra quyền bài học, không tự xác minh JWT.

Realtime và cấu hình local: [Hocuspocus bridge](plans/03c-realtime.md).

## Test API

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Test đặt trong `server/tests/api`, tách theo API và dùng helper dựng app.
Google được giả lập qua HTTP nhưng vẫn kiểm tra chữ ký ID token thật bằng
`openidconnect`. Test auth dùng SQLite riêng; test Places/Courses/Sessions chạy migration thật
trên schema PostgreSQL riêng. Production dùng PostgreSQL. Test phiên chạy với store package, kiểm tra khôi phục sau khi tạo
lại ứng dụng, hết hạn và đăng xuất. Đăng nhập Google thật cần thử riêng trên
trình duyệt với cấu hình Google Console.

Tham khảo: [jsonwebtoken](https://docs.rs/jsonwebtoken/10.4.0/jsonwebtoken/),
[Tower Sessions SQLx store](https://docs.rs/tower-sessions-sqlx-store/0.15.0/tower_sessions_sqlx_store/).

Ảnh và tài liệu: [Private media](plans/03b-private-media.md). Cấu hình `MEDIA_ROOT`
ngoài public root; giá trị local mặc định `.storage/media`. Server dọn file mồ côi
mỗi giờ, chỉ xóa file chưa có metadata và cũ ít nhất 24 giờ. Chạy thủ công bằng
`cargo run -p server --bin media-cleanup`. Khóa file dùng API chuẩn từ Rust 1.89;
filesystem lưu media cần hỗ trợ khóa file.

Bulk deletion và tìm ngày: [hướng dẫn](plans/02d-bulk-and-search.md).

Kho từ vựng từ ghi chú, quyết định chưa dùng queue và lệnh backfill: [hướng dẫn](plans/04-vocabulary-library.md).

Flashcard gõ/chọn, tiến độ riêng và cấu hình IPA/dịch tự động: [hướng dẫn](plans/05-vocabulary-reviews.md).

Bài tập từng node, API và JSON mẫu: [hướng dẫn](plans/06a-inline-exercises.md).

Copy prompt/import nội dung học thêm: [Học cùng AI](plans/07-ai-study.md).

IPA riêng và xóa nhiều nơi/khóa: [hướng dẫn](plans/08-ipa-and-bulk-ui.md).

## Deploy VPS

Cài đặt cá nhân và giờ học mặc định: [hướng dẫn](plans/09-personal-settings.md).

[Hướng dẫn cấu hình PostgreSQL, systemd, Nginx, pgAdmin và GitHub Actions lần đầu](deploy/README.md).

## Kiến trúc hiện tại

[Chuẩn và kế hoạch kiến trúc](plans/10-architecture.md),
[báo cáo hoàn thành và kiểm thử](plans/10-architecture-report.md).
Backend dùng handler/service/DTO, extractor và lỗi JSON chung; frontend theo
chuẩn tại `../pathnote-web/docs/architecture.md`.

[Flashcard gộp theo từ và loại từ](plans/flashcard-word-groups.md): các bài có
cùng từ, locale và loại từ dùng chung lịch ôn, gom các nghĩa để hiển thị.
