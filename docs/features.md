# Tính năng

Hành vi hiện tại của từng tính năng (không phải lịch sử). Luật kiến trúc ở `docs/architecture.md`; việc còn mở ở `docs/roadmap.md`.

Quy ước chung: mọi API dưới `/api/v1`, Bearer JWT (thiếu/hỏng → 401). Không có quyền, sai owner hoặc tài nguyên/cha đã xóa mềm → 404. List trả `{data, meta}`, `page` từ 1, `per_page` 1–100 (mặc định 25 trừ khi ghi khác). Dữ liệu không hợp lệ → 422.

## Nơi học, khóa học, buổi học

### Hành vi

- Ba cấp: Place → Course → Session. Session có thể độc lập (không thuộc khóa), gắn thẳng vào place.
- Chỉ owner thao tác lịch học. Collaborator của note không sửa được lịch.
- PATCH phân biệt field thiếu / `null` / giá trị.
- Session lưu `starts_at` UTC, `duration_minutes`, `timezone`, `status` (enum). Không có sinh lịch tự động.
- Tạo session nhận `initial_note`: `{"mode":"blank"}` (mặc định), `{"mode":"lesson_template"}` (heading "Ghi chú" + heading "Từ vựng" + bảng từ en-US), `{"mode":"structure","session_id":N}` (chỉ chép heading từ buổi khác **cùng khóa**, khác khóa → 422).
- Xóa là xóa mềm, cascade xuống con trong cùng transaction; con đã xóa trước giữ `deleted_at` cũ. Không xóa bytes media hay nội dung note.
- Xóa hàng loạt: 1–100 ID dương, không trùng. Một ID thiếu/đã xóa/sai owner → cả request 404, không xóa phần nào. Thành công 204. Khóa place theo thứ tự tăng dần (tránh deadlock), đọc lại sau khi lấy khóa.
- Mọi thao tác tạo/sửa/xóa con, lưu note, chia sẻ đều khóa place trước → cùng thứ tự khóa với bulk.
- Tìm buổi học: `q` khớp tên (literal, escape `%`, `_`, `!`) **hoặc** ngày `d/m` / `d/m/yyyy` theo `timezone` gửi lên. `d/m` lấy năm hiện tại ở timezone đó. Khoảng ngày = midnight ngày đó → midnight ngày kế tiếp (không cộng 24h, đúng với DST). Ngày sai vẫn dùng làm text. Midnight không tồn tại/không duy nhất → 422.
- Điều kiện OR luôn nằm trong owner scope.
- Lọc search thêm: `place_id`, `course_id`, `independent=true` (không đi cùng `course_id` → 422), `period=past|upcoming`, `order=asc|desc`.
- Neighbors: buổi trước/sau trong cùng khóa (hoặc cùng nhóm độc lập của place), sắp theo `starts_at` rồi `id`.
- Frontend: chọn nhiều place/course/session (tối đa 100), dialog nói rõ cấp con cũng bị xóa; gửi một DELETE chứa cả `ids`. Buổi đang mở phải autosave xong trước khi xóa. Thành công thì reset cache hierarchy/session.

### API

- `GET|POST /places` — list (`q`, phân trang) / tạo.
- `GET|PATCH|DELETE /places/{id}`
- `DELETE /places` — bulk `{"ids":[1,2]}`.
- `GET|POST /places/{place_id}/courses`
- `GET /courses/search` — `q`, phân trang, kèm place qua JOIN.
- `GET /courses/{id}/library-stats` — số buổi, buổi đã qua gần nhất, số từ (vocabularyEntry có word) và số bài tập theo buổi.
- `GET|PATCH|DELETE /courses/{id}`; `DELETE /courses` — bulk.
- `GET|POST /courses/{id}/sessions`; `GET|POST /places/{id}/sessions` (buổi độc lập).
- `GET|PATCH|DELETE /sessions/{id}`; `DELETE /sessions` — bulk.
- `GET /sessions/search` — `q`, `timezone` (bắt buộc), `place_id`, `course_id`, `independent`, `period`, `order`, `page`, `per_page`.
- `GET /sessions/{id}/neighbors`

Code: `server/src/modules/{places,courses,sessions,bulk.rs}`; tìm ngày ở `sessions/service/search.rs`.

## Ghi chú và chia sẻ

### Hành vi

- Mỗi session một note: JSON Tiptap lưu nguyên vào JSONB (không serialize lại struct để tránh thêm field/null). Không cam kết giữ thứ tự key.
- Validate bằng kiểu Serde (`deny_unknown_fields`, `#[serde(tag="type")]`) + luật riêng: heading 1..3, ≤256 KiB, ≤10.000 node, sâu ≤32, bảng không chồng rowspan/colspan, link an toàn.
- Hỗ trợ: text, heading, list/checklist, blockquote, code, bảng, marks, vocabulary, pronunciation, image/file (mediaId), exercise (exerciseId), heading `aiImportId`.
- `schema_version` nhận 1..3; note mới là 3; không cho hạ phiên bản đã tải.
- Note chưa lưu: GET trả paragraph trống, schema 3, revision 0, `saved_at=null`.
- Chống ghi đè: PUT gửi `base_revision`; khớp → revision +1; lệch → 409 kèm `data` là note hiện tại. Hai request cùng revision chỉ một thành công. Frontend giữ draft và cho giải quyết xung đột.
- Reference (media, exercise, AI import) phải thuộc đúng buổi học, sai → 422 (kể cả khi người gọi sở hữu cả hai buổi).
- Lưu note đồng bộ kho từ vựng và đánh dấu AI import attached trong cùng transaction.
- HTTP PUT bị chặn khi realtime bật hoặc note đã có `yjs_state` (xem Realtime).
- Quyền: owner đọc/sửa note + quản lý cộng tác viên. Collaborator đọc/sửa note, không sửa lịch, không quản lý chia sẻ.
- Mời theo email (trim + lowercase) của tài khoản đã từng đăng nhập Google; không gửi email; không mời chính owner; mời lặp không tạo hàng thứ hai.
- Thu hồi idempotent 204; request tiếp theo của người bị thu hồi → 404.
- Save/invite/revoke/xóa cha dùng chung thứ tự khóa place và kiểm tra lại quyền sau khi có khóa.
- Shared list chỉ lấy note được mời, cha còn hoạt động; metadata lấy một query cho cả trang.

### API

- `GET /sessions/{id}/note` → `{data:{session_id,content,schema_version,revision,saved_at}}`.
- `PUT /sessions/{id}/note` — `{"content":{...},"schema_version":3,"base_revision":0}`; 409 khi lệch revision.
- `GET|POST /sessions/{id}/collaborators` — POST `{"email":"..."}`.
- `DELETE /sessions/{id}/collaborators/{user_id}` — 204, idempotent.
- `GET /shared-notes` — phân trang.
- `GET /shared-notes/{id}` — metadata.

Code: `server/src/modules/notes/` (`document/` cho node/attrs/marks/validation, `service/` cho quyền + revision).

## Ảnh và tệp riêng tư

### Hành vi

- Document chỉ lưu `mediaId`; không base64, không path, không URL công khai.
- Owner và collaborator được upload/đọc. Thu hồi/xóa cha → 404 ngay request kế tiếp.
- Ảnh: PNG/JPEG/WebP, ≤5 MiB, mỗi chiều ≤8192 px, decode giới hạn cấp phát 128 MiB. MIME lấy từ bytes, không tin header.
- File: PDF, DOC/DOCX, XLS/XLSX, PPT/PPTX, TXT, ≤10 MiB. Đuôi phải khớp nội dung. TXT là UTF-8, không control ngoài tab/newline. OOXML phải có main part + `[Content_Types].xml` khớp (≤256 KiB, không DTD); không giải nén ra đĩa.
- Tài liệu luôn download dạng attachment. Binary có `private, max-age=31536000, immutable`, `nosniff`, `Vary: Authorization`; frontend dùng blob URL.
- Storage key là UUID; tên hiển thị loại control/path separator; `Content-Disposition` UTF-8 (RFC 5987). Unix: thư mục 0700, file 0600.
- Upload: kiểm tra quyền trước khi đọc multipart và lại sau khi lấy khóa. Ghi file + fsync trước transaction; transaction chỉ khóa cha, kiểm tra quyền, insert metadata. DB lỗi → xóa file khi chắc chắn không có metadata; commit không rõ → giữ file.
- Cleanup mỗi giờ: xóa file tên UUID không có metadata, cũ ≥24h, không đang bị khóa file. Không đụng symlink, file có record, tên lạ. Chạy tay: `cargo run -p server --bin media-cleanup`.
- Xóa node không xóa file (giữ undo/redo, tab khác). Xóa mềm session giữ bytes. Chưa có hard-delete media.

### Cấu hình

- `MEDIA_ROOT` — mặc định `.storage/media`; production dùng đường dẫn tuyệt đối ngoài release (vd `/var/lib/pathnote/media`), backup cùng DB. Filesystem phải hỗ trợ file lock; process cần quyền ghi.

### API

- `POST /sessions/{id}/images` — multipart field `image` → 201 `{data:{id,name,mime_type,size}}`.
- `GET /sessions/{id}/images/{media_id}` — bytes.
- `POST /sessions/{id}/files` — multipart field `file` → 201.
- `GET /sessions/{id}/files/{media_id}` — metadata.
- `GET /sessions/{id}/files/{media_id}/download` — bytes (attachment).

Code: `server/src/modules/media/` (`validation.rs`, `storage.rs`, `cleanup.rs`).

## Realtime (Hocuspocus bridge)

### Hành vi

- Hocuspocus + Yjs lo đồng bộ, phòng tài liệu, awareness. Rust không làm CRDT/WebSocket; chỉ cấp quyền, validate và lưu.
- Frontend lấy cấu hình qua `/collaboration` (không lộ secret), provider dùng JWT hiện có, refresh qua interceptor.
- Hocuspocus gọi authorize với JWT của user + secret; Rust kiểm tra owner/collaborator và cha chưa xóa, thiếu quyền → 404. Quyền/JWT được kiểm tra lại tối đa mỗi 5 giây (thu hồi không tức thì; JWT sống tối đa 15 phút).
- `notes.yjs_state` (BYTEA, nullable). Lần đầu Hocuspocus dựng Yjs từ JSON đúng một lần; sau đó luôn nạp binary đã lưu (dựng lại từ JSON sẽ mất lịch sử/nhân đôi nội dung).
- PUT internal: validate Tiptap, decode base64, chuẩn bị từ vựng ngoài khóa; trong khóa cha kiểm tra quyền, revision/schema, reference. JSONB và binary lưu cùng transaction. Retry cùng nội dung là idempotent. Rust tin binary của bridge, không tự giải mã Yjs.
- Khi bật realtime hoặc note đã có binary: HTTP autosave bị chặn, không fallback âm thầm khi mất kết nối; frontend giữ nội dung và cảnh báo khi rời trang.
- Endpoint internal yêu cầu header `x-collaboration-secret` (so sánh constant-time); secret <32 ký tự hoặc thiếu `COLLABORATION_URL` → bridge tắt (403). Body tối đa 3 MiB.
- Chỉ chạy một Hocuspocus instance (chưa có Redis coordination).

### Cấu hình

- Rust: `COLLABORATION_URL` (vd `ws://localhost:1235`, production WSS), `COLLABORATION_SECRET` (≥32 ký tự).
- Hocuspocus (`pathnote-collaboration`, Node ≥22, file `.env.rust`): `COLLABORATION_API_URL`, `COLLABORATION_API_PREFIX=/api/v1`, `COLLABORATION_SECRET` (cùng secret), `COLLABORATION_HOST`, `COLLABORATION_PORT=1235`, `COLLABORATION_ORIGINS`. Chạy: `node --env-file=.env.rust server/start.mjs`.

### API

- `GET /sessions/{id}/collaboration` — URL WebSocket cho frontend.
- `POST /sessions/{id}/collaboration/authorize` — bridge, JWT + secret.
- `GET /internal/notes/{id}` — bridge đọc note + state.
- `PUT /internal/notes/{id}` — bridge lưu snapshot JSON + Yjs (base64).

Code: `server/src/modules/notes/internal.rs`, `notes/service/`.

## Kho từ vựng

### Hành vi

- Chỉ lấy từ node `vocabulary` (table → `vocabularyEntry` → `vocabularySense` → `vocabularyExample`), không đoán từ văn bản thường. Mỗi sense là một thẻ.
- Thẻ lưu từ, IPA, locale (en-US/en-GB), loại từ, nghĩa, chủ đề, ví dụ, buổi học nguồn. Không sửa JSON gốc khi chuẩn hóa.
- Đồng bộ chạy trong transaction lưu note (cả HTTP lẫn snapshot Hocuspocus): note và kho cùng commit/rollback, không queue, không trạng thái chờ (`syncing=false`).
- Khóa `(session_id, source_sense_id)` giữ ID thẻ khi sửa/undo. Chỉ upsert thẻ đổi nội dung, theo lô 100 (kích thước lô SQL, không phải giới hạn).
- Thẻ biến mất khỏi note → `active=false`; undo kích hoạt lại cùng ID.
- Trim từ/nghĩa/IPA/ví dụ; chủ đề trim + dedup + sort. `word_key` = SHA-256 của từ lowercase + gom whitespace.
- Quyền đọc dùng scope của note: thu hồi chia sẻ hoặc xóa mềm cha → thẻ ẩn ngay, không cần job.
- Progress (gõ/chọn) đọc theo user + `study_key`, không lưu trên thẻ chung.
- Backfill note cũ: `cargo run -p server --bin vocabulary-sync` (trang 100 note, mỗi note một transaction, bỏ qua note đổi revision trong lúc chờ khóa; chạy lại không nhân đôi).

### API

- `GET /vocabulary` — `session_id` (không có quyền → 404), `course_id`, `topic` (≤60, JSONB containment), `q` (≤200, tìm từ hoặc nghĩa, wildcard là ký tự thường), `page`, `per_page` (mặc định 50, tối đa 100). Sort theo word rồi id. `meta`: phân trang, `word_count`, `reviewable`, `topics`, `source`, `syncing`.

Code: `server/src/modules/vocabulary/{sync.rs,library/,rebuild.rs}`, `notes/document/vocabulary.rs`.

## Luyện gõ/chọn từ, IPA và dịch tự động

### Hành vi

- Luyện tập riêng, không ghi vào lịch sử FSRS.
- Gõ từ: hiện nghĩa + IPA + chữ xáo trộn. Chọn từ: 2–4 phương án, khác từ/nghĩa, cùng loại từ + giọng.
- Mỗi câu tối đa hai lần thử. Sai lần đầu: typing trả pattern để điền chữ còn thiếu. Sai lần hai: kết thúc, hiện đáp án. Đúng: hiện từ, IPA, ví dụ.
- Câu hỏi + lần thử lưu JSONB; tiến độ theo `(user_id, card_id)`. F5 tiếp tục được. Retry trùng không cộng điểm lại.
- Chỉ trả câu đang làm; không trả answer key/toàn bộ snapshot. Card đầy đủ chỉ sau khi câu kết thúc.
- Mất quyền hoặc nguồn (kể cả nguồn của phương án nhiễu) bị xóa → cả lượt 404. Từ/nghĩa đổi → câu đang làm 409. Đổi IPA không hủy lượt.
- Progress chỉ dùng khi `study_key` (word/locale/POS/meaning) còn khớp.
- Lịch nhắc đơn giản: đúng lần đầu → streak +1, `due_at` 1/3/7/14/30 ngày; cần lần hai hoặc sai → streak 0, `due_at` +10 phút.
- Tạo lượt lấy mẫu ngẫu nhiên tối đa 1000 candidate + 1000 distractor trong phạm vi quyền, ưu tiên thẻ từng sai.
- IPA/dịch: owner hoặc collaborator gọi. Server gọi dịch vụ ngoài với timeout, không forward JWT/cookie, không follow redirect, giới hạn response. Upstream lỗi/sai cấu trúc/timeout → 503. API không sửa note; editor chỉ áp kết quả nếu field chưa đổi rồi lưu qua luồng note.
- URL rỗng → editor không bật lookup tự động.

### Cấu hình

- `PRONUNCIATION_URL` (vd `http://127.0.0.1:8020`, eSpeak), `TRANSLATION_URL` (vd `http://127.0.0.1:8022`, LibreTranslate). Dịch vụ ở repo `pathnote-pronunciation` (`docker compose up -d pronunciation translation`).

### API

- `POST /vocabulary-reviews` — `mode=typing|choice`, `limit` 1–50 (mặc định 20), lọc `session_id`, `course_id`, `topic`, `q`.
- `GET /vocabulary-reviews/{id}` — chỉ chủ lượt.
- `POST /vocabulary-reviews/{id}/answers` — `position`, `answer`, `attempt=0|1`.
- `GET /vocabulary-reviews/{id}/pronunciation?position=` — từ/locale câu typing để browser đọc.
- `POST /sessions/{id}/pronunciation` — `text` ≤1000 ký tự, locale US/UK.
- `POST /sessions/{id}/pronunciation/batch` — 1–50 texts, ≤64 KiB; dedup khi gọi upstream, trả đúng thứ tự, lỗi từng mục.
- `POST /sessions/{id}/translation` — `text` ≤1000, EN → VI.

Code: `server/src/modules/vocabulary/reviews/`, `server/src/modules/enrichment/`.

## Flashcard FSRS

### Hành vi

- Luồng ôn chính của kho từ. Mặt trước `vi_en`, `en_vi` hoặc `both`.
- Cấu hình (direction + retention) lưu theo user + scope: `(session_id, topic)`, hoặc `(course_id, session_id, topic)` khi lọc theo khóa. `q` chỉ lọc thẻ, không tạo cấu hình riêng.
- Nhóm ôn: user + từ chuẩn hóa (hoa/thường, khoảng trắng kể cả Unicode) + locale + loại từ + chiều. Cùng từ/loại từ ở nhiều bài dùng chung một lịch; các nghĩa đang có quyền gom bằng `; ` (dedup sau chuẩn hóa). Khác loại từ hoặc locale → lịch riêng. Đây là gom theo khóa, không AI suy đoán.
- Đổi từ/locale/loại từ → sang nhóm khác. Đổi nghĩa/IPA/ví dụ/chủ đề → giữ trí nhớ. Ticket kiểm tra hash nội dung nguồn tách khỏi khóa lịch: sửa từ/nghĩa làm ticket đang mở bị từ chối.
- Thứ tự: quá hạn trước, rồi thẻ mới (xáo trộn). Không giới hạn mẫu 1000. Thẻ có lịch tương lai chỉ quay lại khi đến hạn. `new_only` chỉ lấy thẻ mới.
- Learning steps (chính sách ứng dụng): bước 1 phút → 10 phút; Hard ở bước đầu 5,5 phút; relearning một bước 10 phút. Ngoài các bước này dùng interval FSRS, làm tròn ngày, kẹp 1–36500 ngày.
- Crate `fsrs` 6.6.2 (FSRS-6) tính stability/difficulty/interval qua `FSRS::next_states`; không tự viết công thức. Elapsed = số chu kỳ 24h UTC; chưa có day cutoff/fuzz kiểu Anki.
- Retention mặc định 0,90, cho phép 0,70–0,97.
- Next không đánh dấu đã học. Phải reveal trước khi grade (409 nếu chưa). Attempt hết hạn sau 24h. Interval tính tại thời điểm chấm.
- Retry cùng attempt + rating trả kết quả cũ; rating khác → 409. Grade khóa place và user trong transaction: hai tab/hai ticket cùng nhóm chỉ ghi một lượt, tab sau 409. Chiều và người dùng không trộn.
- Không giữ lock khi gọi IPA/audio hay quét queue. Audio dùng giọng trình duyệt.
- Run (lượt ôn): lưu filters + settings lúc tạo, `initial_total` = new (+ due nếu không `new_only`). `next` với `run_id` dùng filters/settings đã lưu; hết thẻ → run tự `completed`; run đã kết thúc → grade 409. Summary: số lượt, số mục duy nhất, đếm theo rating 1–4, thời gian.
- Xóa một nguồn không xóa memory/lịch sử (từ có thể còn ở bài khác); xóa user thì cascade.
- Phím: Space/Enter lật, 1–4 = Quên/Khó/Nhớ/Dễ; chấm xong tự sang thẻ. Nhãn thống kê là "Mục ôn".

### Tự tối ưu tham số

- Worker kiểm tra mỗi phút; không có nút/POST. Advisory lock PostgreSQL đảm bảo một optimizer trên một DB; huấn luyện chạy `spawn_blocking`, không giữ transaction/row lock.
- Ngưỡng: 64 mẫu ôn dài hạn hợp lệ (same-day review vẫn nằm trong chuỗi nhưng không tính vào ngưỡng).
- Chuỗi tách theo user/card/direction/study key và lần bắt đầu lại; chỉ dùng chuỗi bắt đầu từ review 0, liên tục, đã chấm, có timestamp. Không dùng lượt chỉ lật, bài gõ/chọn hay dữ liệu người khác.
- `compute_parameters` fit; so log-loss trên tập huấn luyện bằng `FSRS::evaluate`, chỉ thay khi hợp lệ và thấp hơn model hiện tại.
- Fit lại khi có thêm 64 mẫu, hoặc sau 30 ngày nếu có dữ liệu mới; cách nhau ≥1 giờ. Timeout 90 giây/lỗi → giữ model cũ, thử lại sau.
- Tham số theo người; tiến độ vẫn theo nhóm/chiều. Không gửi dữ liệu ra ngoài.
- Ticket đang mở giữ snapshot tham số. Thẻ mới dùng model mới; memory được replay bằng `FSRS::memory_state` khi model đổi. Không đổi hàng loạt due date hiện tại.
- Tối đa 50.000 review log/người/lượt (vượt → giữ model cũ); prefix huấn luyện ≤256 review (mặc định crate).

### Lịch sử

- Mỗi lượt chấm lưu snapshot word/meaning (các nghĩa đã hiện)/IPA/POS/topics/examples. Xem lại không đổi memory/`due_at`.
- Từ bị gỡ khỏi bảng nhưng bài còn truy cập được → snapshot vẫn xem được. Log cũ thiếu snapshot: chỉ dùng thẻ hiện tại nếu study key còn khớp (UI ghi rõ); nghĩa đã đổi → `card=null`.
- `due_at` hiển thị là lịch đặt **sau lượt đó**, không phải lịch hiện tại.
- Chỉ lượt đã chấm của chính người gọi; vẫn lọc quyền nguồn (xóa mềm/thu hồi → ẩn).
- Lọc ngày theo timezone: hết ngày = đầu ngày kế tiếp tại múi giờ đó (không cộng 24h).

### Migration

- Hai migration chuyển memory từ từng card nguồn sang nhóm từ. Bản sao có lịch khác nhau: giữ trạng thái ôn gần nhất (hòa → nhiều lượt ôn hơn → ID nguồn). Giữ attempt và ID chuỗi gốc để FSRS tái dựng. Ticket thêm hash nội dung + ID chuỗi memory, backfill attempt cũ.
- **Chỉ nâng cấp tiến**: không có down migration tách lại lịch đã gộp.

### API

- `GET /flashcards?session_id=&course_id=&topic=&q=&new_only=` — settings, `word_count`, `reviewable`, `new`, `due`, `learning`, thời điểm đến hạn gần nhất (đếm theo nhóm).
- `PUT /flashcards` — filters + `direction` (`vi_en|en_vi|both`) + `retention` (0,70–0,97).
- `POST /flashcards/next` — filters (+ `run_id`); trả mặt trước + UUID attempt, hoặc `card:null` kèm overview.
- `POST /flashcards/{uuid}/reveal` — đáp án (nghĩa có thể nối `; `) + khoảng ôn dự kiến cho 4 rating.
- `POST /flashcards/{uuid}/grade` — `{"rating":1..4}`.
- `POST /flashcards/runs` — tạo run từ filters → 201 summary.
- `GET /flashcards/runs/{id}`; `POST /flashcards/runs/{id}/finish` — kết thúc sớm (`completed=false`).
- `GET /flashcards/history` — phân trang mới nhất trước; lọc `session_id`, `course_id`, `topic`, `q`, `run_id`, `card_id`, `rating`, `date` hoặc `from`/`to` + `timezone`.
- `GET /flashcards/optimization` — trạng thái, số mẫu, ngưỡng, thời điểm cập nhật.

Code: `server/src/modules/vocabulary/flashcards/` (`queue.rs` chọn/đếm/gom nhóm, `scheduler.rs` gọi `fsrs`, `optimization.rs`, `history.rs`, `personal.rs` tham số theo người, `service/` next/reveal/grade/runs/log).

## Bài tập trong ghi chú

### Hành vi

- Bảy dạng: `choice`, `fill`, `classify`, `matching`, `ordering`, `translation`, `correction`. Mỗi resource đúng một câu, mỗi câu một node `exercise` trong note.
- Note/Yjs chỉ chứa `attrs.exerciseId`; đề và đáp án nằm trong DB. Exercise phải thuộc cùng buổi, sai → 422 khi lưu note.
- Mỗi người có lượt riêng; Yjs không đồng bộ câu trả lời.
- Người tạo đề và owner còn quyền note đọc/sửa definition đầy đủ; người khác chỉ nhận projection không đáp án. Đáp án/giải thích chỉ trả khi câu kết thúc.
- Hai lần thử: sai lần đầu còn một lần; kết thúc khi đúng hoặc sai lần hai. Input không hợp lệ → 422, không tốn lần thử. Retry cùng `answer`/`try` trả lại kết quả cũ.
- Sửa đề tăng revision; trả lời lượt cũ hoặc lượt đã bị restart → 409.
- Giới hạn: title ≤200, prompt ≤2000, option ≤200, answer ≤1000, explanation ≤6000; body ≤256 KiB; import 1–100 câu cùng transaction (`titles` nếu có phải đủ số câu). Field lạ/dạng lạ bị từ chối.
- `choice`/`classify`: 2–6 options khác nhau, một answer nằm trong options. So khớp phương án chính xác theo chuỗi.
- `fill`/`translation`/`correction`: options rỗng, 1–10 đáp án mẫu. So văn bản sau trim + gom whitespace, `caseSensitive` quyết định hoa/thường. Không chấm ngữ nghĩa bằng AI.
- `matching`: 2–8 `items` khác nhau; options/answers cùng số mục, khác nhau, cùng tập; answers theo thứ tự items. Câu trả lời là chuỗi JSON của mảng (≤4000 ký tự), phải đủ cặp, không dùng lại phương án.
- `ordering`: 2–20 từ; câu trả lời phải dùng đúng multiset từ (cho phép trùng). Thiếu/thừa/thêm từ → 422; đúng tập mà sai thứ tự → tính một lần thử.
- Đọc không khóa place; mutation khóa cha theo cùng thứ tự với note/revoke. Validate ngoài transaction, quyền/revision/reference trong transaction.
- Import resource và chèn node là hai bước; đóng trang trước khi lưu note có thể để lại resource không tham chiếu (không xóa ngay để giữ undo). Import không có idempotency key (Học cùng AI có).

Ví dụ import:

```json
{"title":"Bài 1","questions":[
  {"type":"choice","prompt":"She ___ to school.","options":["go","goes"],"answers":["goes"],"explanation":"Ngôi thứ ba số ít.","caseSensitive":false},
  {"type":"matching","prompt":"Nối từ","items":["cat","dog"],"options":["mèo","chó"],"answers":["mèo","chó"],"explanation":"","caseSensitive":false}
]}
```

- Editor gom các block đang tải bằng DataLoader, tối đa 100 ID/request; cache vẫn theo từng bài trong Vue Query. Block mới (import/realtime) tải theo lô mới; trả lời/làm lại cập nhật đúng cache của bài đó, sửa đề chỉ tải lại bài vừa sửa. Hủy một block không hủy request của block khác; hủy tất cả/đổi tài khoản loại bỏ kết quả cũ.

### API

Prefix `/sessions/{session}/exercises`:

- `POST /` — tạo một câu `{title,questions:[1 câu]}` → id.
- `POST /import` — 1–100 câu → ids theo thứ tự.
- `GET /?ids=1,2,...` — đọc theo lô 1–100 ID dương (kể cả ID lặp), loại trùng và giữ thứ tự đầu vào; `{data:[ExerciseResource]}`. Bỏ qua ID không tồn tại/thuộc buổi khác theo cùng quy tắc, buổi không có quyền → 404. Chỉ đề public + lượt mới nhất của chính người gọi ở revision hiện tại; không có chế độ definition. Sau kiểm tra quyền, hai truy vấn lấy đề và lượt làm cho cả lô, không truy vấn trong vòng lặp; dùng index `exercise_attempts_latest` và PostgreSQL `DISTINCT ON`.
- `GET /{id}` — đề public + lượt hiện tại của người gọi.
- `GET /{id}/definition` — đề + đáp án (chỉ người quản lý).
- `PUT /{id}` — sửa, kèm `revision` hiện tại.
- `POST /{id}/attempts` — tiếp tục lượt, `{"restart":true}` để làm lại.
- `POST /{id}/attempts/{attempt}/answers` — `{"position":0,"answer":"...","try":0|1}`.

Code: `server/src/modules/exercises/` (`question.rs` luật từng dạng + projection, `service/`).

## Học cùng AI

### Hành vi

- Slash `/` → Học cùng AI. Frontend tạo prompt từ toàn bộ note (text, bảng từ, IPA/nghĩa/ví dụ, đề bài public) để người dùng dán vào ChatGPT, rồi dán JSON trả về → xem trước → thêm vào cuối bài. Không gọi API AI, không cần API key.
- Prompt không chứa JWT/cookie, ID nội bộ, URL media, đáp án bài tập. File chỉ có placeholder. Note trống thì phải nhập chủ đề.
- JSON nhận heading, paragraph, list, vocabulary (bảng mới, không merge bảng cũ) và bảy dạng bài tập.
- Hai bước:
  1. Client tạo key UUID + payload cố định (section, placeholder slot 1..N, definitions), giữ nguyên qua retry.
  2. `POST .../ai-study/imports`: validate document + câu hỏi ngoài khóa; trong transaction khóa cha, kiểm tra quyền, tạo câu hỏi + receipt `ai_imports`. Trả section không đáp án, heading có `attrs.aiImportId`.
- Retry cùng key + body (cùng user, cùng bài) → cùng IDs/content. Cùng key khác body → 409. Key của user/bài khác → 404.
- Giới hạn: request/document ≤256 KiB, ≤100 câu, slot đủ và duy nhất; không nhận media ref hay marker import khác trong payload.
- Editor kiểm tra marker để không chèn trùng, append bằng một transaction editor, rồi lưu qua autosave/Yjs (không PUT đè tài liệu realtime). Ctrl+Z như node thường.
- Lưu note thành công đánh dấu receipt `attached` cùng transaction với note + kho từ. Conflict/validation rollback thì không. Còn reference tới câu hỏi dù marker bị xóa vẫn tính attached.
- Không phải transaction chung browser–WebSocket–DB: section hiện trong editor chưa có nghĩa là đã lưu. Cache retry không qua F5.
- Cleanup mỗi giờ: tối đa 100 receipt chưa attached, cũ >24h từ lần prepare cuối (retry gia hạn 24h). Cùng parent lock với lưu note; xóa receipt + câu hỏi chưa từng dùng trong một transaction. Receipt đã từng attached giữ vĩnh viễn (phục vụ undo/redo).

### API

- `POST /sessions/{session}/ai-study/imports` — body ≤256 KiB.

Code: `server/src/modules/ai_study/` (`service.rs`, `cleanup.rs`); web `lib/ai-study.ts`, `AIStudyDialog.vue`.

## Trang IPA và block phát âm

### Hành vi

- Route web `/ipa`: bảng 44 âm, audio mẫu UK, giọng trình duyệt US/UK, minimal pairs, tìm kiếm, import JSON.
- Kho IPA riêng theo user + locale; mỗi word/pair là một row JSONB, unique `(user_id, locale, kind, key)`. Không giới hạn tổng số mục.
- GET trả trang 100 row; frontend tải hết các trang của user/locale vào memory để tìm kiếm local (chưa phù hợp kho cực lớn).
- Import ≤256 KiB; pairs 1–50/request. Từ được trim + gom whitespace + lowercase để dedup; US/UK tách riêng. Entry/Sense/Example dùng chung enum locale/POS và validator với node từ vựng.
- Cặp âm: hai âm thuộc bảng 44 âm; IPA hai từ khác đúng một phoneme ở cùng vị trí. Longest match cho âm ghép; alias US quy về âm tham chiếu. Cặp đảo chiều là cùng identity (lưu canonical). Một cặp sai → 422 cả batch.
- Import khóa row user để tuần tự hóa, insert lô 100, unique DB chặn trùng. Chưa có API sửa/xóa kho IPA.
- Block phát âm trong note (`/` → Phát âm): nhập từ/cụm, chọn US/UK, tạo IPA/dịch tự động (debounce/cancel, không áp nếu text/locale/field đã đổi) hoặc nhập tay.
- "Thêm vào từ vựng": thêm vào bảng cùng giọng của bài (tạo bảng nếu chưa có) bằng merge giữ UUID/nội dung; từ đã có giữ loại từ, từ mới dùng other/phrase; ví dụ để trống (không bịa). Kho/flashcard cập nhật khi note lưu. "Ôn tập" mở kho từ lọc theo buổi.

### API

- `GET /ipa-vocabulary?locale=en-US&page=1` — `data.version/locale/entries` + meta.
- `GET /ipa-pairs?locale=en-GB&page=1` — `data.version/locale/sets` + meta; một set có thể trải nhiều trang.
- `POST /ipa-vocabulary/import` — vocabulary JSON v1 → `data.added/skipped`.
- `POST /ipa-pairs/import` — ipa-pairs JSON v1 → `data.added/skipped`.

Code: `server/src/modules/ipa/` (`phonemes.rs`); web `IpaView.vue`, `lib/ipa-library.ts`, `EditorPronunciationView.vue`, `vocabulary-json.ts`.

## Cài đặt cá nhân

### Hành vi

- Bảng `user_settings`, khóa `(user_id, category)`, `value` JSONB. Mỗi nhóm có DTO riêng (`deny_unknown_fields`), lưu bằng upsert; lưu nhóm này không đụng nhóm khác. Không có API ghi JSON tùy ý; user lấy từ JWT.
- Nhóm `learning`: giờ bắt đầu `HH:MM` (giờ địa phương thiết bị, không phải UTC) + thời lượng 1–1440 phút; phải kết thúc trong cùng ngày. Chưa lưu → 19:30 / 60 phút. Chỉ là mặc định cho form tạo buổi mới; không sinh lịch, không sửa buổi cũ.
- Nhóm `navigation`: `hidden_chips` (`{kind:"course"|"place", id}`) và `pinned_places`; mỗi danh sách ≤100, ID dương, không trùng.
- Frontend đọc lại khi mở form tạo buổi, không lưu localStorage/cache chung giữa tài khoản; lỗi tải mặc định vẫn cho nhập tay.

### API

- `GET /settings` — `{data:{learning}}`.
- `PUT /settings/learning` — `{"session_start_time":"18:00","session_duration_minutes":90}`, body ≤4 KiB.
- `GET|PUT /settings/navigation` — `{"hidden_chips":[...],"pinned_places":[...]}`, body ≤16 KiB.

Code: `server/src/modules/settings/`.


## Kiểm thử tự động và điều kiện phát hành

- Web có bộ Playwright Chromium cho desktop 1440×900 và Pixel 7 (touch/user agent), context và cache mới mỗi kịch bản. API fixture có kiểu theo contract thật; dữ liệu/giờ/font cố định, chặn host ngoài loopback và endpoint chưa khai báo.
- Đo 16 màn cùng các thao tác mở form, tìm kiếm, AI copy sau khi cache hết hạn, reveal/grade/next, drawer và buổi trước/sau trên mobile. Mỗi bước có ngân sách request, cấm request trùng (không còn ngoại lệ sau R3) và cấm ghi ngoài lệnh đã khai báo. JSON và bảng CI chỉ rõ request khi vượt ngân sách; tối ưu phải hạ ngân sách cùng commit.
- So sánh ảnh ghi chép, thư viện, flashcard và hộp tạo buổi trên cả hai kích thước. Baseline macOS/Linux riêng; Linux dùng official Playwright image cùng phiên bản Chromium/Ubuntu của CI. Thay ảnh chuẩn phải xem ảnh/diff, không tự cập nhật trong CI.
- Bộ browser contract dùng backend Rust thật và schema PostgreSQL dùng một lần, kiểm tra tạo buổi qua UI, autosave/F5, chấm bài và ẩn đáp án, chia sẻ/thu hồi giữa hai user, lưu lịch ôn flashcard. Harness chỉ nằm trong test binary; JWT test ở file tạm mode 600, không có đường đăng nhập tắt trong production.
- Coordinator checkout đúng backend SHA và frontend SHA từ manifest; kiểm tra request/giao diện và contract phải qua trước deploy backend. Bản build dành cho test dùng loopback, độc lập artifact production. Khi lỗi, CI giữ trace/ảnh/report 7 ngày.
- Phạm vi còn cần thử thật: OAuth Google, realtime hai người, Safari iOS. Mock network không chứng minh độ trễ production; bộ contract không thay phép đo đường truyền/DB trên VPS.

Code: web `e2e/`, `playwright*.config.ts`; Rust `server/tests/api/browser.rs`, `.github/workflows/deploy.yml`. Cách chạy: README web.

- Hiệu năng R1: sao chép prompt AI dùng reader public bulk tối đa 100 ID và cache từng bài; note 24 bài cần 1 GET sau khi cache cũ (trước 24), không đọc definition/đáp án quản lý. Đã đo trên desktop/mobile.

- Hiệu năng R2: places/catalog của dialog đóng không tự tải; mở dialog/drawer mới tải khi cần. Mobile Learn 9 → 7, note 7 → 5; flashcard 5 → 3, cài đặt 3 → 1 (hai kích thước). Hai request chuyển sang thao tác mở Buổi mới/drawer, không bị loại mất chức năng. Timeline vẫn theo dõi đổi tên khóa.

- Hiệu năng R3: next/grade trả thêm `run` thuộc user hiện tại; tổng kết dùng một aggregate SQL thay sáu truy vấn. UI dùng response, các cache khác chỉ đánh dấu cũ đến lúc cần đọc. Startup ôn 4 → 2 API sau R2 (1 GET run + 1 POST next); grade/thẻ kế 5 → 2 (grade + next), reveal vẫn riêng. Giữ ticket, bước xem đáp án, idempotency và FSRS.

- Hiệu năng R4: `GET /courses/library-stats?ids=…` trả summary tối đa 100 khóa, một aggregate SQL theo owner, loại mục đã xóa/mất quyền; không trả note hay danh sách buổi. Parent thư viện tải summary cho nhóm khóa rồi truyền xuống cards. Thư viện desktop 6 → 5 API, mobile 11 → 8 sau R2 (ba nơi/sáu khóa); vào khóa mới tải stats đầy đủ/danh sách buổi.

- Hiệu năng R5: Mở gần đây dùng timeline trước, thiếu metadata mới gọi `GET /sessions/bulk?ids=…` (1–100 ID, đúng owner/đang hoạt động, không có note). Giữ thứ tự local, loại ID không còn truy cập và dùng lại response sau khi loại ID để không tải lặp. Với năm buổi đã có trong timeline, Learn desktop 12 → 7 và mobile 7 → 2 sau R2. Browser kiểm tra cả trường hợp thiếu metadata và ID không tồn tại.
