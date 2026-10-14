# Bước tiếp theo — Courses, relation và transaction

> Tài liệu bước học tại thời điểm triển khai. Trạng thái hiện tại xem
> [Hierarchy](02-learning-hierarchy.md) và [kế hoạch hoàn thiện](06-completion-roadmap.md).

Đã có CRUD courses, phân trang/search, ownership 404 và xóa mềm. Chưa có
sessions, sinh lịch ban đầu hoặc creation key. Frontend vẫn chưa nối API nghiệp vụ.

## Thứ tự đọc

1. `migration/src/m20261013_045811_create_courses.rs`
2. `server/src/entities/courses.rs` và relation Courses trong `places.rs`
3. `server/src/modules/courses/mod.rs`, `handlers.rs`, `dto.rs`
4. `server/src/modules/courses/service.rs`
5. `server/src/modules/places/service.rs`: lock_for_write và delete
6. `server/src/pagination.rs`
7. `server/tests/api/courses.rs`, fixture `learning_helpers.rs`

## Quan hệ: nơi lưu dữ liệu và quyền truy cập

Schema Rust dùng bảng `courses` với `place_id`, không dùng tên bảng learning_paths
của Laravel. Course không lưu user_id thứ hai: chủ sở hữu được xác định qua place.

```text
users.id ← places.user_id
places.id ← courses.place_id
```

`Course belongs_to Place` và `Place has_many Courses` mô tả hai chiều quan hệ.
DeriveRelation của SeaORM dựng RelationDef; QuerySelect cung cấp method join.
Quan hệ không tự áp quyền: query vẫn phải filter user_id và deleted_at của cha/con.

```rust
courses::Entity::find()
    .join(JoinType::InnerJoin, courses::Relation::Place.def())
    .filter(places::Column::UserId.eq(user_id))
    .filter(places::Column::DeletedAt.is_null())
    .filter(courses::Column::DeletedAt.is_null())
```

`select_also(places::Entity)` lấy thêm Model cha từ JOIN đã có. Vì đây là quan hệ
nhiều course → một place, mỗi course vẫn là một dòng, phân trang không nhân bản
record và không cần fetch parent riêng cho từng course.

## Ngày khóa học là calendar date

DTO dùng `chrono::NaiveDate`, schema dùng DATE. Ngày bắt đầu/kết thúc của khóa
không phải thời điểm UTC, không cần chuyển múi giờ. Múi giờ sẽ xuất hiện khi
tạo thời điểm học cụ thể trong bước Sessions.

PATCH vẫn dùng double_option: không gửi ends_on thì giữ ngày cũ; null thì xóa
ngày kết thúc. Sửa starts_on phải so với ends_on đang lưu và ngược lại. Ngoài
validation DTO, migration có CHECK ends_on >= starts_on hoặc ends_on IS NULL.

## Tại sao cần transaction và lock cha?

Có hai thao tác có thể chạy đồng thời:

- Request A kiểm tra place rồi tạo course.
- Request B xóa mềm place cùng các course.

Nếu A chỉ đọc cha một lần mà không phối hợp với B, nó có thể tạo course sau
khi B đã đánh dấu cha là deleted. Cả create/update/delete course và delete place
đều khóa hàng place trước trong transaction. Sau khi chờ lock, phải kiểm tra
lại record còn hoạt động, không dùng snapshot đọc trước transaction.

```rust
let txn = db.begin().await?;
let parent = place_service::lock_for_write(&txn, user_id, place_id).await?;
// insert/update/delete theo nghiệp vụ
txn.commit().await?;
```

SeaORM rollback khi transaction bị drop trước commit, ví dụ validation hoặc
insert trả lỗi qua `?`. Không viết SQL BEGIN/COMMIT thủ công hoặc thêm mutex
trong process. Chưa có bulk-delete/schedule; transaction này chỉ giải quyết
việc phối hợp cha/con hiện có.

`get<C: ConnectionTrait>` cho phép dùng cùng query với DatabaseConnection
hoặc DatabaseTransaction. Đây là generic bound của Rust: caller cung cấp một
kiểu thực hiện giao diện database mà SeaORM định nghĩa.

## Dùng chung phần thực sự lặp

Places và Courses cùng có page, per_page, q và pagination metadata, nên các
kiểu này được chuyển vào `pagination.rs`. `Page<T>` chỉ định kiểu phần tử bằng
generic: Page<PlaceResource> hoặc Page<CourseResource>. Không có generic CRUD
service hay repository framework; nghiệp vụ mỗi module vẫn rõ trong service.

Fixture PostgreSQL của Places đổi tên thành LearningApp để dùng chung cho cả
hai module. Mỗi test có schema riêng `pathnote_learning_test_*`; dữ liệu app
không bị thay đổi.

## API

| Method | URL dưới /api/v1 | Hành vi |
| --- | --- | --- |
| GET, POST | /places/{place_id}/courses | List/tạo trong place được sở hữu |
| GET, PATCH, DELETE | /courses/{id} | Đọc/sửa/xóa mềm course |

Create cần name và starts_on; description và ends_on nullable. Không nhận
place_id/user_id/schedule trong body. Tên trùng vẫn tạo record riêng.
Resource có place {id,name}, không lộ owner ID. Xóa place đánh dấu courses
trực thuộc trong cùng transaction; route trực tiếp vào course cũng trả 404.

## Test đã có

- CRUD với JWT và JOIN ownership, parent metadata.
- PATCH so ngày cũ/mới, missing/null, cấm reparenting.
- Validation trước ghi, phân trang theo place, literal wildcard search.
- CHECK database từ chối ngày không nhất quán ngay cả khi bypass DTO.
- Soft-delete place và courses trong cùng transaction.
- Hai request create course/delete place đồng thời không để course hoạt động
  dưới place đã xóa.

Chạy riêng: `cargo test -p server --test api courses::`.

Tài liệu: [SeaORM relations](https://www.sea-ql.org/SeaORM/docs/1.1.x/relation/one-to-many/),
[SeaORM transactions](https://www.sea-ql.org/SeaORM/docs/1.1.x/advanced-query/transaction/).
