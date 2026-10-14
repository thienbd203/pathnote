# Giai đoạn 2 — Places, Courses, Sessions

Phạm vi hiện tại: CRUD ba cấp, buổi học độc lập, quyền chủ sở hữu, phân trang,
tìm kiếm, xóa mềm và xóa hàng loạt. Sinh lịch học tự động đã được loại khỏi phạm
vi theo quyết định của người dùng; không thêm generator, snapshot hay creation key.

## Những phần đã triển khai

- Places/Courses/Sessions dùng entity, relation và migration SeaORM/SeaQuery.
- JWT xác định chủ sở hữu; sai owner, tài nguyên hoặc cha đã xóa đều trả 404.
- PATCH phân biệt missing/null/value; JSON không có success flag.
- Sessions lưu thời điểm UTC, duration và enum trạng thái; hỗ trợ không thuộc khóa.
- List có page/per_page và tìm tên literal, thứ tự ổn định bằng ID.
- Xóa mềm cha/con trong transaction, không sửa timestamp của con đã xóa.
- Xóa hàng loạt ở cả ba cấp; tìm buổi học theo tên/ngày có timezone.
- Frontend đã nối CRUD, sidebar buổi học, tìm kiếm và chọn nhiều buổi để xóa.
  UI chọn nhiều Places/Courses đã nối ở [bước 8](08-ipa-and-bulk-ui.md).

## Học theo từng bước

1. [Places](02a-places.md): migration, entity, extractor, handler, DTO và service.
2. [Courses](02b-courses.md): relation, JOIN và transaction.
3. [Sessions](02c-sessions.md): Option, enum và thời điểm UTC.
4. [Bulk và tìm ngày](02d-bulk-and-search.md): atomic operation, khóa có thứ tự,
   khoảng ngày ở timezone và OR nằm bên trong ownership scope.

Thư viện xử lý SQL/transaction/phân trang và timezone. Luật chủ sở hữu, xóa cha/con
và contract API là phần ứng dụng; không tạo generic CRUD/repository framework.
Laravel dùng để đối chiếu hành vi, không sao chép tên bảng hay kiến trúc framework.

## Contract chính

```text
GET/POST           /api/v1/places
GET/PATCH/DELETE   /api/v1/places/{id}
GET/POST           /api/v1/places/{id}/courses
GET/PATCH/DELETE   /api/v1/courses/{id}
GET/POST           /api/v1/courses/{id}/sessions
GET/POST           /api/v1/places/{id}/sessions   (buổi độc lập)
GET/PATCH/DELETE   /api/v1/sessions/{id}
DELETE             /api/v1/places                {"ids":[1,2]}
DELETE             /api/v1/courses               {"ids":[1,2]}
DELETE             /api/v1/sessions              {"ids":[1,2]}
GET                /api/v1/sessions/search
```

List trả `{data,meta}`. Bulk nhận 1..100 ID nguyên dương không lặp. Một ID mất,
đã xóa hoặc sai owner làm cả request trả 404, không xóa phần còn lại. Thành công 204.
Search nhận q, timezone, course_id tùy chọn, page/per_page. Nhập d/m hoặc d/m/yyyy
để tìm ngày; d/m dùng năm hiện tại tại timezone đó. Ngày không hợp lệ vẫn được
coi là từ khóa tên, giống hành vi Laravel. Mốc đầu ngày không duy nhất/không tồn
tại trả 422, không tự chọn offset.

## Kiểm tra và giới hạn

API tests chạy trên schema PostgreSQL riêng, không sửa schema ứng dụng.
Bao phủ ownership, PATCH, FK, cascade, rollback logic, concurrency, wildcard,
UTC boundary và ngày DST. Không thêm unit test cho từng helper/service.

Chưa có benchmark dữ liệu lớn hoặc đo query plan trên dữ liệu production.
Không chuyển dữ liệu Laravel sang Rust trong giai đoạn này. Realtime là bước riêng.
