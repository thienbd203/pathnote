# Kiến trúc backend

Chuẩn bắt buộc cho mọi code mới và code được sửa trong `pathnote-rust`. Module mẫu là `server/src/modules/places`; khi phân vân, làm giống `places`.

Frontend có chuẩn tương ứng tại [`../pathnote-web/docs/architecture.md`](../../pathnote-web/docs/architecture.md). Mọi thay đổi JSON mà frontend thấy phải ghi vào bảng [Thay đổi contract API](#thay-đổi-contract-api) ở cuối tài liệu này, trong cùng commit.


## A.1 Layout một module

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
- **Khóa nhiều place trong reviews/answer.rs:** ngoại lệ có chủ đích cho lượt ôn từ nhiều bài. Khóa parent theo ID tăng dần, dùng cùng lock_for_write với lưu note/thu hồi quyền để tránh deadlock và không chấm sau khi mất quyền. Không đổi thành các khóa rời ngoài transaction.
- **Thao tác liên domain:** `modules/bulk.rs` được giữ làm ngoại lệ có chủ đích, vì xóa nhiều place, course hoặc session cần một thứ tự khóa chung. Không thêm file liên domain mới nếu chưa ghi lý do vào tài liệu này.

## A.2 Hạ tầng dùng chung (`server/src/`)

| File | Nội dung | Ghi chú |
| --- | --- | --- |
| `error.rs` | `AppError` và `IntoResponse` | Lỗi duy nhất được trả từ handler |
| `response.rs` | `ApiResponse<T>` (`ok`, `created`, `with_message`) | Giữ field `message` (tùy chọn) cho thông báo thành công từ backend |
| `pagination.rs` | `ListQuery`, `Page<T>`, `PageMeta` | Thêm `PageMeta::new(page, per_page, total)` và `like_contains(term) -> String` |
| `extract.rs` (mới) | `AppJson<T>`, `AppQuery<T>`, `AppPath<T>` | Xem A.3 |
| `access_token.rs` | Extractor `CurrentUser` | Không đổi |

## A.3 Extractor: mọi lỗi đều trả JSON

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

## A.4 Envelope response

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

## A.5 Quyền sở hữu và transaction

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

## A.6 Hiệu năng

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

## A.7 Test

- Mỗi endpoint có test HTTP trong `server/tests/api/<domain>.rs`, dùng helper dựng app chung.
- Test thay đổi query hoặc index chạy trên PostgreSQL (`learning_helpers.rs`), không chạy trên SQLite dựng từ entity, vì SQLite bỏ qua migration và index.
- Mỗi commit phải qua:

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

## Thay đổi contract API

Ghi mỗi thay đổi response/request mà frontend thấy. Cột đầu là đợt hoặc commit.

| Đợt | Endpoint | Trước | Sau |
| --- | --- | --- | --- |
| B.1.1 | mọi route | Lỗi extractor là text thuần, 405 không có body | JSON `{message, errors?}`; path sai trả 404 |
| B.2.8 | `GET /ipa-vocabulary`, `/ipa-pairs` | `meta` thiếu `per_page` | Đủ 4 field |
| B.2.8 | `POST /ipa-*/import` | `{meta:{added,skipped}}` | `{data:{added,skipped}}` |
| B.2.9 | `GET /vocabulary` | `meta.page` | `meta.current_page` |
| B.3.4 / B.4.2 | `GET /flashcards`, `POST /flashcards/next` | Không có số từ/nghĩa; next tính lại ước lượng | Thêm `word_count`, `reviewable`; next giữ overview counts, `estimated_seconds: null` |
| B.4.1 | `GET /courses/{id}/library-stats` | — | Thêm `session_count`, `latest_past_session: {id,title,starts_at} \| null`; lấy cùng truy vấn thống kê hiện có |
| Sau chuẩn hóa: gộp Flashcard theo từ/loại từ | `GET /flashcards`, `POST /flashcards/next`, `POST /flashcards/{id}/reveal` | Đếm/lên lịch từng nghĩa nguồn; hiển thị một nghĩa | Đếm/lên lịch theo từ chuẩn hóa + locale + loại từ + chiều ôn; `front` Việt–Anh và `back.meaning` gom các nghĩa có quyền truy cập bằng `; `. JSON giữ shape. Frontend đổi nhãn `Nghĩa` thành `Mục ôn`. Chi tiết: [features.md](features.md#flashcard-fsrs) |
| Review 11 P1.2 | mọi route quá thời gian | HTTP 408 | HTTP 504, giữ message; upstream dịch 20s, web 28s, route 30s |
| Review 11 P2.9 | `GET /flashcards/runs/{id}` và tổng kết lượt ôn | `unique_reviewed` đếm card nguồn + chiều | Đếm nhóm `study_key` + chiều; một nhóm ôn ở nhiều bài vẫn tính một mục. JSON giữ shape. |
| Bulk đọc bài tập | `GET /sessions/{session}/exercises?ids=1,2,...` | Chỉ có GET từng bài | Thêm `{data:[ExerciseResource]}` tối đa 100 ID dương; loại trùng, giữ thứ tự, bỏ qua ID thiếu/khác buổi; quyền theo note, lượt riêng theo user/revision; dùng cùng public projection và quy tắc ẩn đáp án với GET từng bài. API cũ giữ nguyên. |

| R3 request | `POST /flashcards/next`, `POST /flashcards/{id}/grade` | Không có tổng kết run | Thêm `data.run: RunSummary \| null` theo user của ticket/run; summary là dữ liệu tổng hợp, không lộ đáp án. `next.overview` giữ nguyên; không đổi chấm điểm, khóa hay lịch ôn. |

| R4 request | `GET /courses/library-stats?ids=…` | — | Thêm `{data:[{id,session_count,words,exercises,latest_past_session}]}`; tối đa 100 ID thô, loại trùng/giữ thứ tự, bỏ mục mất quyền hoặc đã xóa; một aggregate SQL, không trả nội dung note/danh sách buổi. API stats từng khóa giữ nguyên. |

| R5 request | `GET /sessions/bulk?ids=…` | — | Thêm `{data:[SessionResource]}` tối đa 100 ID thô, giữ thứ tự sau loại trùng, chỉ metadata buổi thuộc owner đang hoạt động; bỏ ID không tồn tại/mất quyền/đã xóa, không trả note/đáp án. |

| R6 request | `GET /places/bulk?ids=…`, `GET /courses/bulk?ids=…` | — | Thêm `{data:[PlaceResource/CourseResource]}`; cùng validator 1–100 ID thô, giữ thứ tự sau loại trùng, chỉ owner/parent đang hoạt động, bỏ mục không truy cập được. Không mở rộng quyền đọc. |

| R8 request | `POST /ipa-vocabulary/duplicates`, `POST /ipa-pairs/duplicates` | — | Lookup read-only nhận cùng JSON/validator import (giới hạn body 256 KiB), chỉ SELECT các khóa của payload theo user/locale/kind, trả `{data:{duplicates:string[]}}` gồm trùng kho và trong payload; không ghi dữ liệu, không đổi idempotency/khóa import. |

| Hai giọng 21/10 | Note JSON qua REST + bridge | vocabularyEntry/vocabularyExample chỉ có `ipa`, giọng theo bảng `locale` | Nhận thêm `ipaUK?: string|null`, `ipaUS?: string|null` (mỗi trường tối đa 6000 ký tự). Giữ schema_version=3 và IPA/locale cũ cho tương thích. Frontend nhập JSON v1 hỗ trợ hai trường này và ánh xạ IPA cũ đúng locale; không cần migration DB. Deploy Rust và collaboration nhận schema mới trước web. |

| Hai giọng · import IPA | `POST /ipa-vocabulary/{import,duplicates}`, `POST /ipa-pairs/{import,duplicates}` và library | Format từ chỉ có ipa | Chấp nhận và giữ ipaUK/ipaUS tùy chọn trong JSON v1 dùng chung; IPA chính chọn theo locale trước khi phân loại âm. Validation kiểm tra cả hai trường kể cả giọng không được chọn. |
