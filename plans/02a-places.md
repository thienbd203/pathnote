# Bước đầu giai đoạn 2 — Học Rust qua Places

Mục đích của bước này là đọc được toàn bộ một request Rust từ route đến database.
Chỉ triển khai Places; courses, sessions và schedule sẽ mở rộng sau bước này.
Laravel dùng để đối chiếu yêu cầu sản phẩm, không quyết định cấu trúc code Rust.

Đã có CRUD, ownership 404, phân trang, tìm kiếm và xóa mềm. Bốn test HTTP
Places chạy trên PostgreSQL riêng từng schema. Chưa nối frontend hoặc chạy
migration này vào database app.

## Thứ tự đọc code

1. `migration/src/m20261013_043011_create_places.rs`
2. `server/src/entities/places.rs`
3. `server/src/modules/places/mod.rs`
4. `server/src/modules/places/handlers.rs`
5. `server/src/modules/places/dto.rs`
6. `server/src/modules/places/service.rs`
7. `server/tests/api/places.rs`

## Migration: SeaQuery mô tả schema

`DeriveIden` tạo identifier cho tên bảng/cột. `Table::create`, `ColumnDef` và
`ForeignKey::create` dựng câu lệnh bằng API của thư viện; không viết một chuỗi
SQL lớn. `MigrationTrait` là giao diện SeaORM gọi khi chạy `up` hoặc `down`.

Migration tạo bảng `places`, FK đến users và index phục vụ truy vấn theo user.
Chưa tạo trước bảng courses/sessions. Migration users hiện có được giữ nguyên.

## Entity: Model và ActiveModel

`DeriveEntityModel` sinh các kiểu Entity, Column và ActiveModel. `Model` là một
hàng đã đọc từ database. `ActiveModel` mô tả giá trị muốn insert/update.

```rust
places::ActiveModel {
    user_id: Set(user_id),
    name: Set(name),
    ..Default::default()
}
```

`Set(value)` nghĩa là ghi giá trị này. Trường mặc định không được set thì không
trở thành thay đổi PATCH. Đây là API của SeaORM, không phải lớp CRUD tự dựng.
Relation `belongs_to` mô tả place thuộc user; không dùng relation đó để thay
thế kiểm tra quyền truy cập.

## Route, extractor và handler

Route khai báo method/path và handler trong `mod.rs`. Handler nhận dữ liệu bằng
extractor Axum: `State<AppState>`, `CurrentUser`, `Path<i64>`, `Json<T>` và
`Query<T>`. JSON body là extractor cuối vì nó tiêu thụ request body.

`CurrentUser` kiểm tra JWT trước khi handler làm nghiệp vụ. `user.id` đưa vào
service để giới hạn quyền; không đọc `user_id` từ body. Serde DTO dùng
`deny_unknown_fields`, vì vậy body cố gán owner bị từ chối.

Handler trả `Result<ResponseType, AppError>`. Toán tử `?` chuyển lỗi lên caller;
`From<DbErr>` đổi lỗi database thành AppError. `IntoResponse` quyết định HTTP
status và JSON. Đây là luồng lỗi tường minh thay cho exception framework xử lý.

## PATCH có ba trạng thái

`serde_with::rust::double_option` xử lý bằng package:

| JSON | Giá trị Rust | Hành vi |
| --- | --- | --- |
| Không gửi description | `None` | Giữ mô tả hiện có |
| `description: null` | `Some(None)` | Xóa mô tả |
| `description: "text"` | `Some(Some(value))` | Cập nhật mô tả |

Tên không được null. Quy tắc trim và giới hạn độ dài là nghiệp vụ nhỏ, đặt
trong DTO; không dựng generic validation framework cho một module.

## Ownership, pagination và xóa mềm

`owned(user_id)` ghép filter `user_id` với `deleted_at IS NULL`. Không tìm thấy
hàng trong phạm vi này thì trả 404, kể cả ID thực sự tồn tại nhưng thuộc user khác.
List dùng `PaginatorTrait`, page API bắt đầu từ 1, page SeaORM bắt đầu từ 0.

Update/delete cũng có owner và active predicates ngay trong câu lệnh thay đổi,
không chỉ kiểm tra một lần trước đó. Update dùng `exec_with_returning` của SeaORM
trên PostgreSQL để lấy hàng đã sửa. Delete đặt `deleted_at`, không xóa hàng.

Tìm kiếm dùng expression/LIKE có bind parameters của SeaQuery, escape ký tự
wildcard để `%` và `_` là nội dung tìm kiếm. Không dựng SQL bằng user input.

## Những gì chưa thêm

- Ở bước Places ban đầu chưa có bảng con, nên chưa cần transaction/cascade.
- Chưa có bulk-delete; sẽ thêm khi học transaction trong bước tiếp theo.
- Chưa nối giao diện Places sang Rust; frontend hiện vẫn vào trang tài khoản.
- Chưa áp migration Places vào database app. Test dùng schema PostgreSQL riêng,
  có setup/cleanup tường minh; có thể dùng TEST_DATABASE_URL riêng.

Cập nhật khi thêm Courses: xóa place đã có transaction để xóa mềm các course;
phân trang chuyển vào `pagination.rs`, fixture dùng chung là `learning_helpers.rs`.
Đọc [bước Courses](02b-courses.md) cho phần mở rộng này.

## Tài liệu đối chiếu

- [SeaORM: writing migrations](https://www.sea-ql.org/SeaORM/docs/1.1.x/migration/writing-migration/)
- [SeaORM: update](https://www.sea-ql.org/SeaORM/docs/1.1.x/basic-crud/update/)
- [Serde With: double option](https://docs.rs/serde_with/latest/serde_with/rust/double_option/index.html)
