# Pathnote Rust

Axum, SeaORM và PostgreSQL. Google login dùng `openidconnect`; phiên dùng
`axum-login`, `tower-sessions` và `tower-sessions-sqlx-store`. JWT dùng
`jsonwebtoken`, ký RS256 bằng khóa riêng của Pathnote.

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
  "success": true,
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

## Khóa công khai

`GET /api/v1/.well-known/jwks.json` cung cấp khóa công khai với `kid` và `alg=RS256`.
Token có `typ=at+jwt`, `iss=JWT_ISSUER`, `aud=JWT_AUDIENCE`, `sub` là ID user,
`iat`, `exp` và `token_use=access`. Backend kiểm tra chữ ký, thuật toán, key ID,
issuer, audience, loại token và thời hạn. Chỉ có một khóa hoạt động hiện tại;
chưa có cơ chế giữ nhiều khóa để xoay khóa không gián đoạn.

Collaboration và frontend hiện vẫn nối Laravel. Chưa tích hợp Rust vào hai repo
đó và chưa có API quyền bài học trong Rust. JWT hiện có audience `pathnote-api`;
khi thêm collaboration cần thiết kế audience và quyền truy cập riêng.

## Test API

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Test đặt trong `server/tests/api`, tách theo API và dùng helper dựng app.
Google được giả lập qua HTTP nhưng vẫn kiểm tra chữ ký ID token thật bằng
`openidconnect`. Database test là SQLite riêng từng test; production là
PostgreSQL. Test phiên chạy với store package, kiểm tra khôi phục sau khi tạo
lại ứng dụng, hết hạn và đăng xuất. Đăng nhập Google thật cần thử riêng trên
trình duyệt với cấu hình Google Console.

Tham khảo: [jsonwebtoken](https://docs.rs/jsonwebtoken/10.4.0/jsonwebtoken/),
[Tower Sessions SQLx store](https://docs.rs/tower-sessions-sqlx-store/0.15.0/tower_sessions_sqlx_store/).
