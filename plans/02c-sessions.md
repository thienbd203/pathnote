# Học Rust qua Sessions

> Tài liệu bước học tại thời điểm triển khai. Trạng thái hiện tại xem
> [Hierarchy](02-learning-hierarchy.md) và [kế hoạch hoàn thiện](06-completion-roadmap.md).

Đã có CRUD buổi học thuộc khóa và buổi học độc lập. Đây là bước riêng trước
khi thêm sinh lịch tự động; chưa có bulk-delete hoặc tìm kiếm theo ngày.

## 1. Đọc code theo luồng request

1. `migration/src/m20261013_064853_create_sessions.rs`: dùng SeaQuery tạo bảng,
   FK, CHECK và index. Không viết SQL tạo bảng dạng chuỗi.
2. `server/src/entities/sessions.rs`: entity SeaORM và `SessionStatus`.
3. `server/src/modules/sessions/mod.rs`: route.
4. `dto.rs`: kiểu request/response và quy tắc thời gian.
5. `handlers.rs`: extractor JWT, đọc request và chọn mã HTTP.
6. `service.rs`: query owner, JOIN, transaction và thao tác ghi.
7. `server/tests/api/sessions.rs`: test qua HTTP trên PostgreSQL riêng.

CRUD, JSON, transaction, pagination và timezone dùng thư viện. Code ứng dụng
chỉ điều phối các phần đó và thực hiện quy tắc quyền/thời gian của Pathnote.

## 2. Những kiểu Rust đáng học

`course_id: Option<i64>` thể hiện quan hệ tùy chọn. `None` là buổi học độc lập;
`Some(id)` là buổi học trong khóa. Không dùng số 0 làm giá trị đặc biệt.

`SessionStatus` là enum `Scheduled`/`Cancelled`, Serde chuyển thành
`scheduled`/`cancelled` trong JSON, SeaORM chuyển thành chuỗi trong DB.
Trạng thái hủy vẫn đọc được; xóa mềm mới làm buổi học biến mất khỏi API.

PATCH dùng `Option<Option<T>>`: missing giữ nguyên, null trả lỗi cho các
trường bắt buộc, có giá trị thì cập nhật. Không chuyển place/course qua PATCH.

## 3. Thời điểm UTC khác múi giờ

```json
{
  "title": "Lesson 1",
  "starts_at": "2026-10-06T18:00:00+07:00",
  "duration_minutes": 90,
  "timezone": "Asia/Ho_Chi_Minh"
}
```

`DateTime<Utc>` đọc thời điểm này thành `2026-10-06T11:00:00Z`. DB lưu
TIMESTAMPTZ; response trả starts_at và ends_at ở UTC. Không nhận giờ địa phương
thiếu offset vì chưa xác định được một thời điểm duy nhất.

`chrono-tz::Tz` dùng để nhìn thời điểm đó theo ngày địa phương. Quy tắc hiện tại
là buổi học phải kết thúc trong cùng ngày học, không kết thúc đúng 00:00 ngày
sau. Timezone vì vậy cần khi tạo hoặc đổi starts_at/duration, không cần khi
chỉ sửa tên/trạng thái. Timezone không được lưu như một thuộc tính của session.
Cả `Asia/Saigon` và `Asia/Ho_Chi_Minh` được thư viện nhận.

Cộng duration trên UTC đo thời gian thực. Ví dụ New York ngày đổi giờ mùa xuân:
01:30 -05:00 cộng 90 phút là 04:00 -04:00. Bước sinh lịch sau này mới cần
`from_local_datetime` để xử lý giờ địa phương không tồn tại hoặc bị lặp.

Thời lượng 1..1440 phút và độ chính xác tối đa microsecond theo PostgreSQL.
Buổi học bù có thể nằm ngoài khoảng ngày khóa học; thời gian của khóa dùng cho
lịch ban đầu, không cấm buổi học thủ công ngoài lịch.

## 4. Quan hệ, quyền và transaction

Session JOIN place để giới hạn owner, LEFT JOIN course vì course có thể null.
`select_also` lấy metadata hai cha trong cùng query; paginator làm count và lấy
trang, không tải riêng cha cho từng session.

Composite FK `(course_id, place_id)` bảo đảm khóa thuộc đúng nơi học của buổi
học. FK đơn theo course_id sẽ không giữ được điều này. Khi course null, FK
place vẫn kiểm tra nơi học tồn tại.

Create/update/delete khóa place trước, rồi kiểm tra lại parent/session trong
transaction. Xóa course đánh dấu các buổi học trực thuộc; xóa place đánh dấu
cả buổi học độc lập. Cùng thứ tự lock giúp không để con hoạt động dưới cha vừa
bị xóa khi hai request chạy đồng thời. Không ghi lại deleted_at của con đã xóa.

## 5. API và kiểm tra

- GET/POST `/api/v1/courses/{id}/sessions`: buổi học trong khóa.
- GET/POST `/api/v1/places/{id}/sessions`: chỉ buổi học độc lập.
- GET/PATCH/DELETE `/api/v1/sessions/{id}`: thao tác một buổi học.
- List: `page`, `per_page`, `q` tìm tên không phân biệt hoa thường;
  `%` và `_` là ký tự literal; thứ tự starts_at rồi ID.
- Create trả 201; read/update trả 200; delete trả 204.
- Không có JWT trả 401; sai owner hoặc đã xóa trả 404; dữ liệu sai trả 422.

```sh
cargo test -p server --test api sessions::
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Sáu test Sessions kiểm tra CRUD, scope, quyền, PATCH, timezone/DST, cascade,
FK và cạnh tranh create/delete. Không áp migration vào DB ứng dụng khi chạy
test: helper dùng schema riêng rồi dọn schema đó.

Tham khảo:
[SeaORM relation](https://www.sea-ql.org/SeaORM/docs/1.1.x/relation/one-to-many/),
[transaction](https://www.sea-ql.org/SeaORM/docs/1.1.x/advanced-query/transaction/),
[chrono-tz 0.10.4](https://docs.rs/chrono-tz/0.10.4/chrono_tz/).
