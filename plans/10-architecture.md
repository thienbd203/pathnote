# Chuẩn kiến trúc backend và kế hoạch chuẩn hóa

Ngày chốt: 16/10/2026. Tài liệu có hai phần:

- **Phần A** là chuẩn bắt buộc cho mọi code mới và code được sửa.
- **Phần B** là danh sách việc để đưa code hiện tại về chuẩn. Mỗi task nhỏ, làm và commit riêng.

Module mẫu là `server/src/modules/places`. Khi phân vân, làm giống `places`.

Frontend đi kèm có chuẩn riêng tại `../pathnote-web/docs/architecture.md`. Mọi thay đổi contract API phải ghi trong mục B.6 để frontend cập nhật cùng lúc.

---

## Phần A — Chuẩn

### A.1 Layout một module

```text
server/src/modules/<domain>/
├── mod.rs        chỉ khai báo mod con và fn routes() -> Router<AppState>
├── handlers.rs   extract input → gọi service → trả response
├── service.rs    nghiệp vụ, truy vấn SeaORM, transaction, kiểm tra quyền
└── dto.rs        struct request (Deserialize) và resource (Serialize), validate_*
```

Quy tắc:

- **`handlers.rs`:**
  - Không có SQL, không `begin()` transaction và không `json!`.
  - Mỗi handler chỉ có một lời gọi service. Ngoại lệ: gọi HTTP ngoài (enrichment) sau khi service xác nhận quyền.
- **`service.rs`:**
  - Nhận `&C: ConnectionTrait` (hoặc `&DatabaseConnection` khi cần mở transaction), `user_id: i64` và input đã parse.
  - Trả model, DTO hoặc `AppError`. Không biết gì về axum.
- **`dto.rs`:**
  - Request: `#[serde(deny_unknown_fields)]`. PATCH dùng `serde_with::rust::double_option` để phân biệt thiếu field với `null`.
  - Resource: tên `<Name>Resource`, có `impl From<Model>`.
- **Service lớn:** khi một service vượt khoảng 300 dòng, đổi thành thư mục `service/` và tách theo hành động (`service/create.rs`, `service/answer.rs`), như `vocabulary/reviews` đang làm.
- **Domain con:** dùng cùng layout lồng bên trong, ví dụ `vocabulary/flashcards/{mod,handlers,service,dto}.rs`.
- **Route nội bộ** (bridge collaboration): đặt trong `<domain>/internal.rs`, có router và middleware riêng, nhưng vẫn gọi service. Không ghi DB trực tiếp trong file đó.
- **Thao tác liên domain:** `modules/bulk.rs` được giữ làm ngoại lệ có chủ đích, vì xóa nhiều place, course hoặc session cần một thứ tự khóa chung. Không thêm file liên domain mới nếu chưa ghi lý do vào tài liệu này.

### A.2 Hạ tầng dùng chung (`server/src/`)

| File | Nội dung | Ghi chú |
| --- | --- | --- |
| `error.rs` | `AppError` và `IntoResponse` | Lỗi duy nhất được trả từ handler |
| `response.rs` | `ApiResponse<T>` (`ok`, `created`, `with_message`) | Giữ field `message` (tùy chọn) cho thông báo thành công từ backend |
| `pagination.rs` | `ListQuery`, `Page<T>`, `PageMeta` | Thêm `PageMeta::new(page, per_page, total)` và `like_contains(term) -> String` |
| `extract.rs` (mới) | `AppJson<T>`, `AppQuery<T>`, `AppPath<T>` | Xem A.3 |
| `access_token.rs` | Extractor `CurrentUser` | Không đổi |

### A.3 Extractor: mọi lỗi đều trả JSON

Không dùng `Result<Json<T>, JsonRejection>` rồi `map_err` trong từng handler nữa. Thay bằng extractor của axum với `rejection(AppError)`. Cần bật feature `macros` cho axum trong `Cargo.toml` gốc.

```rust
// server/src/extract.rs
use crate::AppError;
use axum::extract::{rejection::*, FromRequest, FromRequestParts};

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct AppQuery<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
pub struct AppPath<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(e: JsonRejection) -> Self { AppError::validation("body", &e.body_text()) }
}
impl From<QueryRejection> for AppError {
    fn from(e: QueryRejection) -> Self { AppError::validation("query", &e.body_text()) }
}
// An ID that cannot be parsed is treated like a missing record.
impl From<PathRejection> for AppError {
    fn from(_: PathRejection) -> Self { AppError::NotFound("Không tìm thấy dữ liệu.".into()) }
}
```

Một handler theo chuẩn:

```rust
pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    AppPath(id): AppPath<i64>,
    AppJson(input): AppJson<UpdatePlace>,
) -> Result<ApiResponse<PlaceResource>, AppError> {
    Ok(ApiResponse::ok(service::update(&state.db, user.id, id, input).await?.into()))
}
```

- **Thông báo lỗi riêng:** handler cần thông báo khác (ví dụ note dùng field `content`) thì vẫn nhận `Result<AppJson<T>, AppError>` và đổi lỗi tại chỗ. Đây là ngoại lệ, phải có comment một dòng giải thích.
- **405 trả JSON:** trong `app.rs`, thêm `.method_not_allowed_fallback(...)` trả `AppError` với status 405 (thêm biến thể `MethodNotAllowed`).

### A.4 Envelope response

| Trường hợp | Status | Body |
| --- | --- | --- |
| Một bản ghi | 200 / 201 | `{ "data": T, "message"?: string }` qua `ApiResponse::ok` / `created` |
| Danh sách có phân trang | 200 | `{ "data": [T], "meta": { "current_page", "last_page", "per_page", "total" } }` qua `Page<T>` |
| Danh sách không phân trang | 200 | `{ "data": [T] }` qua `ApiResponse::ok(vec)` |
| Kết quả của lệnh (import, bulk) | 200 | `{ "data": { ... } }`, ví dụ `{ "data": { "added": 3, "skipped": 1 } }` |
| Xóa | 204 | Không có body |
| Lỗi | 4xx/5xx | `{ "message": string, "errors"?: { field: [string] }, "data"?: T }` |

Quy tắc:

- **`meta`:**
  - Luôn có đủ bốn field phân trang và tên giống hệt bảng trên.
  - Số liệu phụ (ví dụ thống kê của kho từ) đặt thêm trong `meta` bằng tên riêng. Không đổi tên bốn field chuẩn.
- **Không dùng `serde_json::Value` / `json!` trong response.** Mỗi response là một struct `Serialize` trong `dto.rs`. Chỉ nội dung note Tiptap (vốn là JSON tự do) được phép giữ `Value`.
- **Lỗi kèm dữ liệu** (409 khi lưu note, cần trả bản mới nhất): dùng `AppError::ConflictWith { message, data: serde_json::Value }`. Body vẫn đúng shape lỗi ở bảng trên, nên handler không tự dựng response.
- **Kiểu dữ liệu:**
  - Thời điểm: RFC3339 UTC.
  - ID: số `i64`, trừ ticket hoặc attempt dùng UUID.
  - Enum: `snake_case`.
- **Status:**
  - 401: chưa đăng nhập.
  - 404: không tồn tại hoặc không thuộc user. Không phân biệt hai trường hợp, để không lộ dữ liệu.
  - 403: chỉ dùng khi origin hoặc nguồn không hợp lệ.
  - 409: xung đột phiên bản.
  - 422: validation.
  - 503: service ngoài lỗi.

### A.5 Quyền sở hữu và transaction

- **Phạm vi quyền:**
  - Mỗi domain có `owned(user_id) -> Select<Entity>` trong service. Mọi đọc và ghi bắt đầu từ đó.
  - `courses::owned` và `sessions::owned` join lên place, như hiện tại.
  - Note dùng `notes::service::accessible` (owner hoặc collaborator).
- **Ghi có khóa:**
  - Dùng một helper chung trong `places::service`:

    ```rust
    pub(in crate::modules) async fn write_tx(
        db: &DatabaseConnection, user_id: i64, place_id: i64,
    ) -> Result<DatabaseTransaction, AppError>
    ```

    Helper này `begin()` rồi `lock_for_write()`. Caller kiểm tra lại quyền của bản ghi con trong transaction rồi `commit()`.
  - Không chép lại cặp `begin` + `lock_for_write` ở từng module.
- **Khóa nhiều place:** khóa theo thứ tự `id` tăng dần để tránh deadlock, như `bulk.rs` đang làm.
- **Không gọi HTTP ngoài trong transaction** (pronunciation, translation, AI). Kiểm tra quyền trước, gọi HTTP sau, rồi mở transaction mới nếu cần ghi.
- **Việc tốn CPU hoặc IO đồng bộ** (decode ảnh, zip, đọc file) chạy trong `tokio::task::spawn_blocking`.

### A.6 Hiệu năng

- **Không truy vấn trong vòng lặp:**
  - Đọc nhiều bản ghi: `is_in(ids)`.
  - Ghi nhiều bản ghi: `insert_many`.
- **Endpoint danh sách chỉ select cột cần dùng**, qua `select_only().columns(...)` hoặc `into_partial_model`. Không bao giờ load `notes.content` hoặc `notes.yjs_state` trong danh sách hay metadata.
- **Không lặp truy vấn trong một request:**
  - Service nhận model đã load thay vì load lại theo ID.
  - Kiểm tra quyền một lần ngoài transaction và một lần trong transaction (sau khi khóa) là đủ.
- **Index:**
  - Mỗi cột FK, mỗi cột dùng trong `WHERE`, `JOIN` hoặc `ORDER BY` của endpoint có tải lớn đều có index trong migration.
  - Index nhiều cột sắp theo thứ tự filter rồi sort.
- **Tìm kiếm** dùng `pagination::like_contains`. Khi bảng lớn mới thêm `pg_trgm`, không làm trước.
- **Middleware chung** trong `app.rs`:
  - `CompressionLayer` (tower-http, feature `compression-br,compression-gzip`).
  - `TimeoutLayer` 30 giây cho route không upload.
  - Request ID (`SetRequestIdLayer` + `PropagateRequestIdLayer`), đưa vào span của `TraceLayer`.
- **Pool DB:** `max_connections` và `acquire_timeout` đọc từ env (`DATABASE_MAX_CONNECTIONS`, mặc định 10; `DATABASE_ACQUIRE_TIMEOUT_SECS`, mặc định 5).
- **Cache-Control:**
  - JSON giữ `no-store`.
  - Ảnh và tệp theo `media_id` là bất biến (thay ảnh sẽ tạo ID mới), nên trả `private, max-age=31536000, immutable`.
  - Download stream file bằng `tokio_util::io::ReaderStream` thay vì đọc cả file vào RAM.

### A.7 Test

- Mỗi endpoint có test HTTP trong `server/tests/api/<domain>.rs`, dùng helper dựng app chung.
- Test thay đổi query hoặc index chạy trên PostgreSQL (`learning_helpers.rs`), không chạy trên SQLite dựng từ entity, vì SQLite bỏ qua migration và index.
- Mỗi task trong Phần B phải qua:

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

---

## Phần B — Việc cần làm

Thứ tự là bắt buộc. Mỗi task là một commit. Không gộp refactor với thay đổi hành vi trong cùng commit. Task đánh dấu **[contract]** thay đổi JSON mà frontend thấy, nên phải cập nhật bảng B.6.

### B.1 Hạ tầng

1. **Extractor và lỗi chung**
   - Bật `axum` feature `macros`.
   - Tạo `extract.rs` (A.3), thêm `AppError::MethodNotAllowed` và `AppError::ConflictWith`, gắn `method_not_allowed_fallback`.
   - Thêm test: body JSON sai, query sai, path `abc` và method sai đều trả JSON đúng shape.
2. **Helper phân trang và tìm kiếm**
   - Thêm `PageMeta::new` và `like_contains` vào `pagination.rs`.
   - Thay 7 đoạn escape LIKE chép tay: `places/service.rs:41`, `courses/service.rs:72`, `sessions/service.rs:99`, `sessions/search.rs:117`, `notes/service.rs:251`, `vocabulary/library.rs:179`, `vocabulary/flashcards/log.rs:104`.
   - Thay 5 chỗ tự dựng `PageMeta`.
3. **`ApiResponse.message`:** giữ field, thêm builder `with_message(self, message: impl Into<String>) -> Self` để handler có thể trả thông báo thành công, ví dụ `ApiResponse::ok(note).with_message("Đã lưu.")`. Field chỉ được serialize khi có giá trị, nên response hiện tại không đổi.
4. **Middleware**
   - Compression, timeout và request ID như A.6. Cấu hình pool đọc từ env, thêm biến vào `.env.example`.
   - Upload media và import ra khỏi timeout 30 giây, hoặc đặt timeout riêng 120 giây.
5. **Helper `places::service::write_tx`**
   - Thay cặp begin + lock ở: `exercises/service.rs:18-28`, `media/service.rs:83-84`, `notes/service.rs:107-110` và `:183-185`, `vocabulary/flashcards/mod.rs` reveal/grade, update/delete của sessions và courses.

### B.2 Đưa module về layout chuẩn

Mỗi dòng dưới đây là một commit. Hành vi không đổi, test cũ phải qua mà không cần sửa assert.

1. **`places`, `courses`, `sessions`:** đổi sang `AppJson`, `AppQuery` và `AppPath`. Việc này cũng làm mẫu cho các module sau.
2. **`courses/library.rs`:** chuyển SQL vào `courses/service.rs`. Handler chỉ còn gọi service.
3. **`sessions/search.rs`:** chuyển phần dựng query vào `sessions/service.rs` (hoặc `sessions/service/search.rs`), DTO vào `dto.rs`.
4. **`notes`:**
   - Tách `collaboration.rs` thành route người dùng (vào `handlers.rs`) và `internal.rs` (bridge, middleware `bridge_secret`). Phần ghi DB vào `service.rs`.
   - 409 dùng `AppError::ConflictWith`, body giữ nguyên `{message, data}`.
5. **`media`:** tạo `dto.rs`, chuyển `MediaResource` và `Kind` vào đó.
6. **`enrichment`:** tạo `dto.rs` cho request và response, `service.rs` cho kiểm tra quyền và gọi client.
7. **`exercises`, `ai_study`:** chuyển handler từ `mod.rs` sang `handlers.rs`, thay `json!` bằng struct.
8. **`ipa`, `settings`:** tách `mod.rs` thành handlers, service và dto. **[contract]** IPA list thêm `meta.per_page`, IPA import trả `{data:{added,skipped}}`.
9. **`vocabulary/library.rs`:**
   - Tách handler và service, dùng `ListQuery::page_size` thay check riêng.
   - **[contract]** `meta.page` đổi thành `meta.current_page`.
10. **`vocabulary/reviews`:** chuyển handler từ `mod.rs` sang `handlers.rs`, thay `json!` bằng struct.
11. **`vocabulary/flashcards`:** chuyển insert và transaction trong `mod.rs` (`:144-163`, `:216-305`), `log.rs` và `runs.rs` vào service. Handler chỉ còn gọi service.

### B.3 Hiệu năng (sau B.2)

1. **Migration index mới:**
   - `ai_imports(session_id)`
   - `ai_imports(user_id)`
   - `exercises(user_id)`
   - `flashcard_attempts(user_id, reviewed_at DESC)`

   Test history và overview chạy trên PostgreSQL.
2. **`shared-notes`, `notes::metadata`, `collaboration::config`:** chỉ select `session_id, saved_at` (và cột thật sự cần). Không load `content` hoặc `yjs_state`.
3. **Lưu note** (`PUT /sessions/{id}/note`): chỉ còn hai lần kiểm tra quyền (ngoài và trong transaction) và một lần đọc note. Truyền model đã load giữa các hàm.
4. **`POST /flashcards/next`:**
   - Authorize một lần, đọc memory một lần.
   - Gộp các COUNT/MIN theo direction thành một truy vấn `GROUP BY direction` với `COUNT(*) FILTER (...)`.
   - Không tính lại cả overview cho mỗi thẻ. Response chỉ trả số đếm cần cho UI.
   - Đo số truy vấn trước và sau (bật `sqlx_logging` khi test) và ghi kết quả vào commit message.
5. **`GET /vocabulary`:** gộp 7 truy vấn thành tối đa 3 (page, total, stats bằng `FILTER`).
6. **`GET /sessions/{id}/neighbors`:** lấy hai neighbor trong cùng truy vấn tìm ID, không gọi `get` lại.
7. **`exercises/service.rs:108-121`:** thay insert từng câu bằng `insert_many`.
8. **`vocabulary/reviews/answer.rs:53-59`:** load các place bằng `is_in`, khóa theo thứ tự ID tăng dần.
9. **Media:**
   - Download stream bằng `ReaderStream`.
   - Đổi Cache-Control của ảnh và tệp như A.6. Cần `tokio-util` với feature `io`; kiểm tra xem crate đã có trong dependency tree chưa trước khi thêm.

### B.4 Endpoint gộp để giảm request từ frontend

1. **[contract] Tóm tắt khóa học**
   - Mở rộng `GET /courses/{id}/library-stats` (hoặc `CourseResource` trong list theo place) với `session_count` và `latest_past_session: {id,title,starts_at} | null`.
   - Mục tiêu: frontend bỏ hai lời gọi `/sessions/search?per_page=1` cho mỗi thẻ khóa học (`LibraryCourseCard.vue:23`).
   - Chọn mở rộng list theo place nếu làm được trong một truy vấn (`LEFT JOIN LATERAL`). Nếu không, mở rộng library-stats.
2. **[contract] Flashcard overview nhẹ**
   - `GET /flashcards` hiện được ba màn hình gọi để lấy số đếm.
   - Đảm bảo endpoint này chỉ đọc số đếm (không trả danh sách thẻ) và dùng chung truy vấn đã tối ưu ở B.3.4.

### B.5 Không làm trong đợt này

`pg_trgm`, cache Redis, tách crate, đổi framework hoặc ORM, đổi cơ chế auth, đổi schema note.

### B.6 Thay đổi contract (frontend phải theo)

| Task | Endpoint | Trước | Sau |
| --- | --- | --- | --- |
| B.1.1 | mọi route | Lỗi extractor là text thuần, 405 không có body | JSON `{message, errors?}`; path sai trả 404 |
| B.2.8 | `GET /ipa-vocabulary`, `/ipa-pairs` | `meta` thiếu `per_page` | Đủ 4 field |
| B.2.8 | `POST /ipa-*/import` | `{meta:{added,skipped}}` | `{data:{added,skipped}}` |
| B.2.9 | `GET /vocabulary` | `meta.page` | `meta.current_page` |
| B.3.4 / B.4.2 | `GET /flashcards`, `POST /flashcards/next` | Không có số từ/nghĩa; next tính lại ước lượng | Thêm `word_count`, `reviewable`; next giữ overview counts, `estimated_seconds: null` |
| B.4.1 | `GET /courses/{id}/library-stats` | — | Thêm `session_count`, `latest_past_session: {id,title,starts_at} \| null`; lấy cùng truy vấn thống kê hiện có |
| Sau chuẩn hóa: gộp Flashcard theo từ/loại từ | `GET /flashcards`, `POST /flashcards/next`, `POST /flashcards/{id}/reveal` | Đếm/lên lịch từng nghĩa nguồn; hiển thị một nghĩa | Đếm/lên lịch theo từ chuẩn hóa + locale + loại từ + chiều ôn; `front` Việt–Anh và `back.meaning` gom các nghĩa có quyền truy cập bằng `; `. JSON giữ shape. Frontend đổi nhãn `Nghĩa` thành `Mục ôn`. Chi tiết: `plans/flashcard-word-groups.md` |

### Điều kiện hoàn thành

- `rg "JsonRejection|QueryRejection" server/src/modules` chỉ còn trong các ngoại lệ có comment.
- `rg "json!\(" server/src/modules` chỉ còn trong `notes/document`.
- Không còn `.begin().await` đi liền `lock_for_write` ngoài `write_tx`.
- Mọi module có `mod.rs` chỉ chứa khai báo và `routes()`.
- Ba lệnh ở A.7 đều qua. Frontend đã cập nhật theo bảng B.6.
