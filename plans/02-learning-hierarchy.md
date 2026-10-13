# Giai đoạn 2 — Places, courses, sessions

Trạng thái: **đang triển khai theo bước**. CRUD Places, Courses và Sessions đã có;
bulk-delete, tìm ngày học và sinh lịch ban đầu chưa triển khai.

Sau khi review cách tiếp cận, triển khai từng bước theo cấu trúc Rust thay vì
copy schema/kiến trúc Laravel. Bắt đầu với [Places](02a-places.md); cấu trúc module
và tên bảng bên dưới là đề xuất ban đầu, không ràng buộc code phải sao chép nó.

## 1. Mục tiêu và phạm vi

Triển khai cây dữ liệu `user → place → course → session`, cùng buổi học độc lập
`user → place → session`. Chỉ chủ sở hữu được đọc hoặc thay đổi dữ liệu;
tài nguyên không tồn tại, đã xóa hoặc thuộc người khác đều trả `404`.
Thiếu hoặc sai access token trả `401`, không chuyển trường hợp này thành `404`.

Phạm vi gồm CRUD ba cấp, phân trang, tìm kiếm, xóa mềm, xóa hàng loạt và tạo
khóa học kèm lịch ban đầu. Không gồm note/editor, sharing, collaboration,
flashcard, thông báo lịch, tái sinh lịch hoặc khôi phục dữ liệu đã xóa.
`cancelled` là trạng thái buổi học, khác với xóa mềm.

Laravel hiện có CRUD đọc/tạo/sửa ba cấp, tìm kiếm và bulk soft-delete sessions.
Xóa mềm places/courses, xóa đơn và bulk-delete places/courses là **phần mở rộng
của kế hoạch này**, không coi là hành vi đã có trong Laravel.

## 2. Mục tiêu học

| Nội dung | Bài thực hành |
| --- | --- |
| Relation, join trong SeaORM | Truy vấn course/session qua place của user; lấy metadata cha không gây N+1 |
| Transaction và row lock | Tạo course + toàn bộ sessions; xóa hàng loạt; đồng bộ thao tác tạo con với xóa cha |
| Phân trang | `PaginatorTrait`, count và thứ tự ổn định, chuyển page API từ 1 sang page nội bộ từ 0 |
| `chrono`, `chrono-tz` | Tách ngày lịch, giờ địa phương và thời điểm UTC; xử lý DST bằng kết quả của thư viện |
| Tổ chức service | Handler nhận request, service điều phối nghiệp vụ, generator chỉ xử lý lịch |
| HTTP API tests | Kiểm tra quyền, lỗi, rollback và cạnh tranh qua endpoint; không test riêng từng helper |

## 3. Nguồn Laravel đã đối chiếu

Các đường dẫn dưới đây tính từ repo `../pathnote-api`:

- `app/Http/Controllers/LearningPlaceController.php`,
  `LearningPathController.php`, `LearningSessionController.php`.
- `app/Services/CreateLearningPathService.php`, `InitialScheduleGenerator.php`.
- `app/Policies/LearningPlacePolicy.php`, `LearningPathPolicy.php`,
  `LearningSessionPolicy.php`.
- `app/Http/Requests/SaveLearningPlaceRequest.php`,
  `SaveLearningPathRequest.php`, `SaveLearningSessionRequest.php`.
- Ba resource tương ứng và các migration `learning_places`, `learning_paths`,
  `learning_sessions`, `initial_schedule`, `deleted_at`.
- `SessionSearchController.php`, `SessionBulkDeleteController.php`.
- `tests/Feature/LearningHierarchyTest.php`, `InitialScheduleTest.php`,
  `LearningSessionTest.php`, `SessionSearchTest.php`, `SessionBulkDeleteTest.php`.

Policy chia sẻ note trong Laravel chưa áp dụng ở giai đoạn này: collaborator
không được coi là chủ sở hữu của course hoặc session.

## 4. Tổ chức code theo module Rust

```text
server/src/modules/places/    # mod.rs, handlers.rs, dto.rs, service.rs
server/src/modules/courses/   # mod.rs, handlers.rs, dto.rs, service.rs
server/src/modules/sessions/  # bước sau, chưa tạo
server/src/entities/          # Model, ActiveModel, Relation
server/src/pagination.rs      # ListQuery và Page<T> dùng chung
migration/src/                # mỗi bước schema một migration SeaQuery
server/tests/api/             # HTTP test theo từng feature
```

Chỉ tạo module khi triển khai tính năng tương ứng. Giữ workspace hiện tại;
chưa tạo generic CRUD/repository framework. Service có transaction khi cần
phối hợp nghiệp vụ, không chỉ vì controller Laravel từng gọi service.

## 5. Thiết kế dữ liệu dự kiến

Schema Rust dùng `places`, `courses` và sau này `sessions`, thống nhất với
module và API. Laravel chỉ đối chiếu hành vi. Các trường schedule/creation key
sẽ thêm bằng migration riêng khi làm generator, không tạo trước ở bước CRUD.
Đây chưa phải kế hoạch chuyển dữ liệu Laravel.

| Bảng | Trường nghiệp vụ chính |
| --- | --- |
| `places` | `id`, `user_id`, `name`, `description` |
| `courses` | `id`, `place_id`, `name`, `description`, `starts_on`, `ends_on`, `initial_schedule`, `creation_user_id`, `creation_key`, `creation_fingerprint` |
| `sessions` | `id`, `place_id`, `course_id` nullable, `title`, `starts_at`, `duration_minutes`, `status` |

Mỗi bảng có `created_at`, `updated_at`, `deleted_at` nullable. ID dùng bigint
như users hiện tại; ngày khóa học dùng PostgreSQL `DATE`; thời điểm dùng
`TIMESTAMPTZ`, serialize UTC. `initial_schedule` dùng JSONB.
`ends_at` tính từ start + duration, không lưu thêm một cột trùng dữ liệu.

Ownership chỉ nằm ở place. Course thuộc place; session phải thuộc cùng place
với course nếu có course. Dùng FK và, nếu schema phù hợp, composite FK
`(course_id, place_id)` để khóa điều kiện nhất quán này.
Không tin `user_id` hoặc các parent ID do body gửi lên.

Index dự kiến:

- Place: `(user_id, id)` cho bản ghi chưa xóa.
- Course: `(place_id, id)` cho bản ghi chưa xóa.
- Session: `(course_id, starts_at, id)` và
  `(place_id, course_id, starts_at, id)` cho bản ghi chưa xóa.
- Unique `(creation_user_id, creation_key)` khi có key; **không bỏ unique khi
  course bị soft-delete**, để retry không tạo lại tài nguyên đã xóa.
- FK restrict hard-delete; xóa mềm cha/con thực hiện trong transaction.

Không thêm index cho tìm kiếm contains theo cảm tính. Bắt đầu query có bind
parameters và owner filter; xem `EXPLAIN` trên dữ liệu đại diện trước khi thêm
trigram/index phục vụ search.

## 6. Quyền truy cập và HTTP contract

API nghiệp vụ dùng `CurrentUser` từ JWT. Truy vấn course/session phải join
place và lọc `user_id`, cùng `deleted_at IS NULL` ở mọi cấp cha liên quan.
Đưa điều kiện này vào queries dùng chung để tránh quên ownership ở endpoint mới.
Việc đã xác thực không đồng nghĩa được truy cập một ID bất kỳ.

Trước khi thêm POST/PATCH/DELETE nghiệp vụ, thu hẹp kiểm tra Origin hiện tại
về các route dựa trên cookie phiên. Bearer API không bắt buộc có Origin;
CORS frontend vẫn giữ riêng. Kiểm tra cả trường hợp token hợp lệ không có
Origin và refresh cookie từ origin không được phép.

Các endpoint dự kiến dưới `/api/v1`:

| Method | Path | Công việc |
| --- | --- | --- |
| GET, POST | `/places` | Danh sách/tạo nơi học |
| GET, PATCH, DELETE | `/places/{id}` | Đọc/sửa/xóa mềm |
| GET, POST | `/places/{id}/courses` | Danh sách/tạo khóa, có thể kèm lịch |
| GET, PATCH, DELETE | `/courses/{id}` | Đọc/sửa/xóa mềm |
| GET, POST | `/courses/{id}/sessions` | Buổi học trong khóa |
| GET, POST | `/places/{id}/sessions` | Buổi học độc lập, không có course |
| GET, PATCH, DELETE | `/sessions/{id}` | Đọc/sửa/xóa mềm buổi học |
| DELETE | `/places`, `/courses`, `/sessions` | Bulk-delete, body `{ "ids": [...] }` |
| GET | `/sessions/search` | Tìm theo title hoặc ngày học, tùy chọn course |

List places/courses có `q` để tìm theo name; sessions search giữ contract
Laravel với `q`, `course_id`, `timezone`, `page`. Route search phải được phân
biệt rõ với route ID.

- Đọc/sửa `200`, tạo `201`, xóa `204` không body.
- `401` xác thực, `404` tài nguyên, `409` creation key khác nội dung,
  `422` validation với `{ "message": "...", "errors": { "field": ["..."] } }`.
- Không có `success` hoặc `failed` trong response.
- List trả `{ "data": [...], "meta": { "current_page", "last_page", "per_page", "total" } }`.
  Default list 25, search sessions 20; đề xuất cho phép `per_page` tối đa 100.
  Page bắt đầu từ 1, không hợp lệ trả 422; page vượt cuối trả danh sách rỗng.
- Order places/courses theo ID; sessions theo `starts_at`, rồi ID.
- Resource giữ metadata place/course như Laravel, không lộ user ID sở hữu,
  creation key/fingerprint hoặc note content.

## 7. Validation và cập nhật

- Trim name/title/description đầu cuối; tên rỗng sau trim không hợp lệ.
  Name/title tối đa 200 ký tự, description tối đa 2000.
- PATCH phải phân biệt **không gửi**, **gửi null**, **gửi giá trị**. Không dùng
  một `Option<T>` đơn giản nếu nó làm mất phân biệt missing/null.
- `starts_on` là YYYY-MM-DD, `ends_on` nullable và không trước start. Khi sửa
  một trường phải so với trường còn lại đang lưu.
- Session start yêu cầu RFC3339 có offset hoặc Z; lưu instant UTC. Duration
  1..1440, trạng thái `scheduled` hoặc `cancelled`.
- Tạo session hoặc sửa giờ/duration cần timezone để kiểm tra kết thúc trong
  cùng ngày học địa phương. Timezone không thay thế offset trong `starts_at`.
- Chấp nhận timezone alias trình duyệt như `Asia/Saigon` theo khả năng của
  `chrono-tz`; xác minh bằng test, không giả định chỉ nhận tên canonical.
- Không cho đổi parent qua PATCH. Buổi học bù có thể nằm ngoài ngày khóa học.
- Sửa metadata khóa không tự sinh lại sessions. `schedule`, `creation_key` và
  các trường nội bộ không được gửi qua PATCH.
- JSON thiếu/sai type cũng trả validation contract nhất quán; kiểm tra cơ chế
  rejection của Axum trước khi viết wrapper tùy chỉnh.

## 8. Sinh lịch ban đầu và chống tạo trùng

Input tham khảo Laravel:

```json
{
  "name": "Foundation",
  "starts_on": "2026-09-29",
  "ends_on": "2027-03-31",
  "creation_key": "<UUID do client giữ cho cùng một lần tạo>",
  "schedule": {
    "timezone": "Asia/Ho_Chi_Minh",
    "until": "2027-03-31",
    "entries": [
      { "weekday": 2, "starts_at": "19:30", "duration_minutes": 90 },
      { "weekday": 4, "starts_at": "19:30", "duration_minutes": 90 }
    ]
  }
}
```

Quy tắc và thứ tự:

1. Kiểm tra owner của place trước khi xử lý tạo. Schedule optional; nếu có
   thì bắt buộc creation key UUID. Key của mỗi user là độc lập.
2. Validate và chuẩn hóa dữ liệu, sort entries theo weekday/giờ rồi tạo
   fingerprint xác định từ place ID, metadata course và schedule. Thứ tự
   entries trong request không làm thay đổi fingerprint; dùng serialization
   canonical rõ ràng, không hash object có thứ tự key tùy ý.
3. Nếu key đã tồn tại: cùng fingerprint trả course đã tạo, không chạm lịch
   đã chỉnh; khác fingerprint trả 409. Nếu course hoặc parent đã xóa, trả
   404, không tạo lại. Giữ response 201 cho replay thành công như Laravel.
4. Sinh từng ngày lịch địa phương, bao gồm start và until. ISO weekday
   1=Monday..7=Sunday; 1..21 entries. Until không trước start, không sau end
   nếu course có end, và tối đa 2 năm lịch từ start. Course không có end vẫn
   cần until hữu hạn. Không dùng 730 ngày để thay thế 2 năm lịch.
5. Dùng `chrono-tz::Tz` và `TimeZone::from_local_datetime`; chỉ nhận
   `LocalResult::Single`. `None` hoặc `Ambiguous` trả 422 ở đúng entry,
   không tự chọn offset hoặc tự đẩy giờ. Không viết thuật toán DST thủ công.
6. Cộng duration theo thời gian thực UTC rồi kiểm tra ngày kết thúc địa
   phương; không cho vượt sang ngày sau, kể cả kết thúc đúng 00:00 hôm sau.
   Kiểm tra overlap cả ở entries và danh sách UTC sau chuyển đổi; hai ca
   sát nhau, ca trước kết thúc đúng lúc ca sau bắt đầu, được phép.
7. Không có ngày khớp lịch hoặc sinh quá 500 buổi trả 422, chưa ghi dữ liệu.
   Sort sessions theo UTC, đặt `Session 01`, `Session 02`… và lưu snapshot
   `initial_schedule` có `starts_on`, `session_count`.
8. Transaction khóa place còn hoạt động, tạo course và toàn bộ sessions.
   Lỗi ở bất kỳ insert nào rollback tất cả. Tạo manual session cũng dùng
   quy tắc khóa cha nhất quán để không tạo dưới parent vừa bị xóa.
9. Unique constraint là bảo đảm cuối cùng cho request đồng thời. Nếu conflict,
   rollback transaction đã lỗi rồi đọc lại course trong transaction mới và
   áp dụng kiểm tra fingerprint. Không tiếp tục query trong transaction
   PostgreSQL đã bị abort; không dùng mutex trong một instance thay cho unique.

Fixture chuẩn: lịch thứ Ba/thứ Năm trong JSON trên sinh **53 buổi**; buổi đầu
`2026-09-29T12:30:00Z`, buổi cuối `2027-03-30T12:30:00Z`, duration 90 phút.

## 9. Xóa mềm, bulk-delete và tìm kiếm

Thiết kế đề xuất cho phần mở rộng:

- Xóa session giữ hàng dữ liệu với `deleted_at`; list/detail/search không thấy.
- Xóa course đánh dấu course và sessions trực thuộc trong cùng transaction.
- Xóa place đánh dấu place, courses và mọi sessions thuộc place, kể cả độc lập.
- Không sửa lại `deleted_at` của con đã xóa trước đó. Chưa cung cấp restore.
- Bulk nhận 1..100 ID nguyên dương, không lặp. Một ID mất, đã xóa hoặc sai owner
  thì toàn batch trả 404 và không xóa phần nào. Chỉ trả 204 sau commit.
- Khóa parent/child theo một thứ tự được thống nhất giữa create và delete;
  sort ID trước khi lock để giảm deadlock. Không hard-delete hoặc cascade dữ
  liệu note/media nếu sau này thêm các quan hệ đó.
- Search lọc owner và active parents trước, không giới hạn tìm trên trang đã
  tải. `q` trim, tối đa 120 ký tự; `%`, `_` và escape character là text literal.
- Search sessions tùy chọn course phải xác thực ownership bằng 404.
- `d/m` hoặc `d/m/yyyy` hợp lệ có thể khớp ngày học theo timezone. Không có
  năm dùng năm hiện tại tại timezone đó. Khoảng ngày là `[đầu ngày, đầu ngày
  kế tiếp)` chuyển sang UTC; không cộng cố định 24 giờ vì DST.
- Title match và ngày match nằm trong cùng nhóm OR bên trong owner filter.
  Dùng bind parameters; không nối user input thành SQL.
- Mốc ngày search không chuyển đổi được duy nhất thì trả 422 theo chính sách
  timezone rõ ràng, không panic. Thời gian hiện tại có thể inject qua helper
  test để test `d/m` không thay đổi theo năm chạy.

## 10. Test API và database

Không thêm unit test cho từng service/helper. Dùng endpoint để kiểm tra các
luồng và lỗi; fixture/setup không nhất thiết đi qua Google cho mỗi test.
Helper có thể tạo user và JWT test bằng signing library hiện có.

Tests migration, FK, transaction, row lock và concurrent creation phải dùng
**PostgreSQL riêng cho test**, không dùng SQLite để kết luận về concurrency.
Dùng schema/database riêng từng test, cùng schema migration nghiệp vụ thật,
seed owner A/B và cleanup đúng phần test. Không đụng database local đang học.
Xác minh package dựng PostgreSQL test có sẵn trước khi tự viết fixture hạ tầng;
không thêm nhiều framework test nếu helper hiện có mở rộng được.

Nhóm coverage bắt buộc:

- JWT thiếu/hỏng/hết hạn; valid Bearer không có cookie và không có Origin.
- CRUD mỗi cấp, parent metadata, trim, missing/null PATCH, ngày không hợp lệ.
- Đọc/sửa/xóa/tạo con dưới parent của user khác đều 404; list không lộ record.
- Session độc lập; mismatch place/course; đổi giờ giữ ID/title; cancel khác delete.
- List nhiều trang, order tie-breaker, không N+1 theo số lượng record.
- Soft-delete cha/con và bulk toàn bộ rollback khi lẫn ID của user B hoặc mất ID.
- Search ngoài trang đầu, scope course, UTC boundary, literal wildcard, ngày DST.
- Fixture 53 buổi; năm nhuận; open-ended course; không có ngày khớp; trên 500.
- DST spring gap/fall fold bị từ chối; lịch qua DST giữ giờ địa phương và đổi UTC offset.
- Same key/same payload; entries đảo thứ tự; payload/place khác trả 409;
  khác user dùng cùng key; replay không ghi lại sessions đã sửa hoặc đã xóa.
- Hai request cùng key đồng thời chỉ tạo một course và một bộ sessions.
- Lỗi insert session ở giữa rollback course và sessions trước đó. Fault injection
  chỉ trong fixture test, không thêm failure switch vào production API.
- Concurrent create child/delete parent không để lại child hoạt động dưới cha bị xóa.

## 11. Thứ tự triển khai

1. [ ] Chốt contract DTO, lỗi 422/pagination, query owner, các đề xuất soft-delete.
2. [x] Thu hẹp Origin check về cookie auth; kiểm tra giữ nguyên bảo vệ refresh/logout.
3. [x] Migration, entities, relation/FK/index; chuẩn bị PostgreSQL API test fixture.
4. [x] Places CRUD và courses CRUD không schedule; test quyền và PATCH.
5. [x] Course sessions và independent sessions; thời gian UTC và validation timezone.
6. [ ] Generator chrono-tz, create-course transaction, snapshot và idempotency.
7. [ ] Xóa mềm ba cấp, xóa đơn và bulk; khóa cha/con và rollback.
8. [ ] Search/phân trang; kiểm tra query count và kế hoạch query trên dữ liệu đại diện.
9. [ ] Đối chiếu test Laravel, chạy API suite, fmt, Clippy và migration trên DB test mới.
10. [ ] Viết hướng dẫn API cho frontend; việc nối UI sang API nghiệp vụ là bước riêng.

## 12. Điều kiện hoàn thành

- Tất cả endpoint trong phạm vi có API tests qua trên PostgreSQL test.
- Mọi truy vấn dữ liệu giới hạn owner/active parents; không có lỗ truy cập qua ID.
- Lịch không sai UTC/DST, không sinh trùng khi retry/concurrent, rollback toàn bộ khi lỗi.
- Xóa mềm và bulk không xóa một phần, không rò dữ liệu qua search hoặc truy vấn con.
- HTTP contract không có success flag; frontend dùng được metadata phân trang.
- Migration chạy được từ DB test trống, test không phụ thuộc dữ liệu cá nhân.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  và test suite tương ứng đều qua; cập nhật trạng thái tài liệu bằng bằng chứng đó.

## 13. Tài liệu thư viện cần dùng

Theo SeaORM 1.1.x đang dùng, tránh lấy ví dụ API SeaORM 2.x đưa vào trực tiếp:

- [SeaORM 1.1.x](https://www.sea-ql.org/SeaORM/docs/1.1.x/index/)
- [Chrono LocalResult: Single/Ambiguous/None](https://docs.rs/chrono/latest/chrono/offset/enum.LocalResult.html)
- [Chrono TZ](https://docs.rs/chrono-tz/latest/chrono_tz/)

Trước khi thêm `chrono-tz` hoặc crate hỗ trợ validation/fingerprint/test,
kiểm tra version tương thích, bảo trì và nhu cầu thật. Dùng primitive thư viện
cho timezone, transaction và pagination; chỉ tự viết quy tắc nghiệp vụ của Pathnote.
