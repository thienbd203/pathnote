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
- Mở ghi chép owned/shared: frontend tải metadata và cấu hình realtime song song, dùng chung query observer để không refetch khi mount note. Editor realtime tải module trong lúc socket đồng bộ, chỉ mount sau khi metadata thành công và Y.Doc đã sync; không đổi contract/lưu trữ hay bỏ xác thực.
- Hiệu năng bài dài (22/10): bảng tái sử dụng hàng không đổi; realtime không xuất toàn bộ JSON hay dựng snapshot/hash theo từng ký tự. Hash vẫn kiểm đầy đủ trạng thái Yjs khi server xác nhận lưu (gồm deletion/undo), bỏ kết quả quá hạn khi có sửa mới. Bản nháp xuất theo yêu cầu giữ nội dung mới nhất. Không thay schema, contract, migration hay dữ liệu ghi chú. Số đo fixture và cách chạy profile ở `pathnote-web/docs/ui.md`.
- Bảng từ vựng trong editor dùng ba cột Từ / Nghĩa / Ví dụ, loại từ nằm trong ô nghĩa; cuộn ngang trên mobile. Toggle IPA câu là lựa chọn hiển thị theo tài khoản, mặc định bật, lưu localStorage và áp dụng mọi bảng; khi sửa câu hoặc có lỗi IPA thì vẫn hiện. Nút sửa phụ xuất hiện theo hover/focus, luôn hiện trên thiết bị không có hover; bản chỉ đọc giữ nghe và toggle. UK và US luôn có hai dòng loa + IPA riêng dưới từ/câu. Bảng chỉ cấu hình tốc độ, giọng đọc chọn tự động theo locale; nếu thiết bị chưa có giọng tương ứng thì báo rõ. Note/JSON v1 nhận ipaUK/ipaUS tùy chọn, giữ ipa/locale cũ ở đúng giọng; cross-locale import gộp hai IPA mà giữ bản sửa tay. Enrichment dùng endpoint cũ với hai locale, debounce/chống kết quả cũ áp dụng từng IPA; batch chỉ điền phần còn thiếu. Collaboration giữ hai trường qua Yjs và presence. Kho từ/flashcard hiện tiếp tục dùng IPA chính theo locale của bảng. Cần deploy Rust + collaboration trước frontend, không có DB migration.
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
- `PATCH /sessions/{id}/files/{media_id}` — `{name}` → metadata mới; tên tối đa 200 ký tự, không rỗng/ký tự điều khiển/đường dẫn/dấu nháy kép, giữ nguyên đuôi file.
- `GET /sessions/{id}/files/{media_id}/download` — bytes (attachment).

Code: `server/src/modules/media/` (`validation.rs`, `storage.rs`, `cleanup.rs`).

## Block nghe và xem MP3/M4A/MP4 trong editor

- Thiết kế **Gọn trong trang**: block ở giữa nội dung ghi chú, tên file/MIME/dung lượng, Phát/Tạm dừng, thanh tua/thời gian, tốc độ; không có dòng Tiếp tục từ. MP4 có video và fullscreen. Không tự phát sau upload/F5/undo. Slash `/mp3`, `/m4a`, `/mp4`, `/nghe`, `/video`, menu + hoặc paste/drop file để chèn; khung upload chỉ chọn/thả file từ máy, không hiển thị danh sách tệp đã tải trong buổi. Khung chọn file hiện ngay tại đoạn gõ lệnh và không lưu vào JSON note; hủy khung giữ nội dung xung quanh. Không còn khung Nghe & xem riêng phía trên ghi chú.
- Dùng node `file` atom đã có, chỉ lưu `{mediaId}` trong JSON/Y.Doc, không thêm schema/attribute. Vẫn schema_version 4; frontend cũ biết file nên giữ tham chiếu, có thể chỉ hiện dạng tải tệp. Metadata phải thuộc đúng buổi, backend kiểm khi lưu như mọi file khác. Byte file, tên và MIME nằm trong media/storage riêng tư, không lưu URL công khai/vé phát/duration/progress/rate vào document.
- Menu Tải xuống lưu file gốc bằng tên hiện tại, dùng endpoint tải file có xác thực; người chỉ đọc vẫn tải được.
- Menu Đổi tên file giữ nguyên đuôi; tên mới áp dụng mọi block tham chiếu cùng file. Không đổi storage key, bytes, MIME, tiến độ hoặc revision note. Người có quyền sửa buổi được đổi tên; tab khác cần tải lại metadata.
- Thay file cập nhật mediaId, xóa chỉ xóa node; kéo/sắp xếp/nhân bản/undo/redo dùng cơ chế editor hiện có, giữ các đoạn xung quanh. Sau undo, click/chạm đoạn văn cập nhật con trỏ ngay để thao tác gõ nhanh trên mobile không ghi đè block vừa khôi phục. File không bị xóa vật lý nên undo/tab khác tiếp tục truy cập nếu còn quyền. Các block đã chèn tiếp tục tham chiếu file cũ bình thường. Chưa có hard-delete media.
- MP3/M4A/MP4 tối đa 100 MiB mỗi file, multipart field file qua endpoint upload hiện có. `infer` kiểm header và đối chiếu đuôi; M4A được nhận diện bằng header và lưu/stream với MIME `audio/mp4`; M4A/MP4 cần codec trình duyệt hỗ trợ, chưa chuyển mã server. Ảnh giữ 5 MiB, tài liệu văn phòng/TXT/PDF giữ 10 MiB. Upload mới/thay file bị hủy khi editor/node bị tháo; vị trí chèn được map qua transaction và recheck khi request xong.
- Chèn mới MP3/M4A/MP4 (23/10): chọn hoặc thả nhiều file; frontend upload tuần tự từng request, giữ thứ tự và hiện tiến độ từng file. Cả nhóm chèn bằng một transaction khi upload xong, undo cùng nhau. Lỗi giữa nhóm cho phép retry từ file lỗi mà không upload lại file đã xong; hủy không chèn nhóm dở dang, các file đã tải vẫn được giữ trên server. Validation toàn bộ nhóm trước upload; không đổi API/schema/giới hạn mỗi file. Thay thế một block vẫn chỉ chọn một file.
- HTTP Range/HEAD do tower-http ServeFile cung cấp; không tải toàn bộ thành blob để phát/tua. Vé RSA có type riêng, scope user/session/media, hạn 4 giờ, không dùng làm API bearer. Mọi request stream kiểm lại quyền buổi đang hoạt động; thu hồi chia sẻ chặn request tiếp theo, không thu lại buffer đã tải.
- Tiến độ nằm ở media_progress, khóa (user_id,media_id), không chia sẻ giữa người học. Player đọc tiến độ riêng từng file và metadata để khôi phục, không autoplay. Ghi định kỳ 5 giây khi phát, pause/tua/ended, ẩn/đóng trang và tháo block/rời bài. Nghe hết lưu 0 để nghe lại. Các write nối tiếp; lỗi lưu hiện thông báo và nút thử lại. Hai thiết bị hoặc hai block cùng file dùng lần ghi cuối, chưa merge tiến độ.
- Tiến độ là trạng thái cá nhân, không làm đổi note revision, JSON hay Y.Doc. Source và load bị bỏ khi tài khoản/generation đổi. Trong cùng editor chỉ phát một block tại một thời điểm. Note chỉ đọc vẫn phát được, không hiện thao tác thay/xóa.
- Gửi lúc đóng trang là best effort (keepalive); tắt máy/kill trình duyệt/mất mạng có thể mất phần sau lần lưu thành công gần nhất (thường tối đa 5 giây nếu mạng ổn định). Không cam kết lưu được khi offline hoặc request đang lỗi.
- Migration `m20261021_034622_add_lesson_recordings` tạo bằng sea-orm-cli, chỉ thêm bảng tiến độ; block file dùng schema đã có, không cần migration bổ sung.

API dưới `/api/v1`, cần bearer trừ stream dùng vé:

- `GET /sessions/{id}/recordings` → `{data:[{id,name,mime_type,size,position_seconds}]}` cho thư viện chọn file trong buổi.
- `POST /sessions/{id}/recordings/{media}/playback` → `{data:{ticket}}`.
- `GET|HEAD /sessions/{id}/recordings/{media}/stream?ticket=…` → bytes 200/206, Range sai 416; không vé/khác scope 401, mất quyền 404; private/no-store, nosniff, inline.
- `GET /sessions/{id}/recordings/{media}/progress` → `{data:{position_seconds}}`, mặc định 0 nếu chưa nghe, kiểm quyền + cùng buổi + MIME MP3/M4A/MP4.
- `PUT /sessions/{id}/recordings/{media}/progress` body `{position_seconds}` → `{data:{position_seconds}}`, số hữu hạn 0..604800; lưu riêng người gọi, không sửa note.

Kiểm thử: fixture tổng hợp MP3/M4A/MP4 bằng FFmpeg; API/PostgreSQL kiểm upload giả, giới hạn cũ, Range/416, scope/vé hết hạn, quyền/thu hồi, tiến độ riêng, file reference cùng buổi và note revision bất biến khi nghe. Browser desktop/mobile kiểm chèn giữa đoạn văn, F5, tua/rate không sửa document, thay MP4, xóa/undo và chèn lại file; integration dùng backend thật. Unit kiểm retry/thứ tự write và node chỉ có mediaId.

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
- Regression 22/10: một entry có hai nghĩa cùng loại từ vẫn chỉ có một mục ôn theo mỗi chiều; cả hai nghĩa xuất hiện trên thẻ, chấm Nhớ không lấy ngay nghĩa còn lại thành thẻ mới. Bật cả hai chiều có hai lịch độc lập; ghi chú giữ nguyên hai nghĩa.
- Đối chiếu read-only trên production 22/10: cấu hình Việt → Anh/97%, 164 từ và 190 nhóm ôn. Trong các lượt gần đây, rest 08:57:18 là động từ và rest 08:57:55 là danh từ; nguồn ghi chú có hai loại từ riêng (walk, sleep cũng vậy). believe chấm Quên 08:56, hẹn 08:57, rồi ôn lại 08:57; prepare chấm Nhớ 08:55, hẹn 09:05. Các lượt này khớp grouping và learning steps; chưa tái hiện lặp cùng từ/locale/POS/chiều ngay sau Nhớ/Dễ. Lịch sử hiện chưa hiện loại từ cạnh từ nên dễ nhầm hai nhóm thành thẻ trùng. Không chấm thẻ/đổi cấu hình khi đối chiếu.
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

### Phát âm mặc định UK (25/10)

- Block phát âm/bảng từ vựng mới, mẫu bài mới, JSON mẫu và giọng đọc mặc định dùng `en-GB`. Bảng từ vựng vẫn giữ hai nút UK/US để nghe đối chiếu; locale được ghi rõ trong note/import cũ được bảo toàn.
- Thư viện từ vựng, lượt ôn gõ/chọn mới và mặt trước/đáp án flashcard dùng giọng UK. IPA từ/câu lấy từ `ipaUK`; chỉ dùng `ipa` cũ làm dự phòng khi nguồn vốn là `en-GB`. Dữ liệu chỉ có IPA Mỹ thì phần IPA UK để trống, không đổi nhãn thành UK. Giọng UK phải có trên thiết bị; không tự chọn giọng Mỹ khi thiếu giọng UK.
- Migration `m20261025_010001_british_pronunciation` thêm `vocabulary_cards.ipa_uk`, lấy UK và IPA câu từ note hiện có, kể cả block lồng nhau. Khi lưu note tiếp, projection cập nhật cả IPA UK/US của câu. Không sửa nội dung/revision/Yjs của note, không thay `locale` nguồn, khóa nhóm, memory, due date hoặc lịch sử FSRS. Snapshot/lượt ôn đã tạo trước release giữ phát âm lịch sử của chúng; lượt mới dùng UK.
- `locale` trong dữ liệu trả cho thư viện/ôn mới là locale phát âm; locale nguồn dùng nội bộ để giữ identity cũ. Không thêm query theo từng từ, không gọi dịch vụ IPA hàng loạt trong migration. Triển khai migration + Rust trước collaboration/frontend như workflow hiện có; không cần cấu hình VPS mới.

### Thống kê thời gian ôn (24/10)

- Flashcard có hai mục **Ôn tập** / **Thống kê** (`/flashcards?view=statistics`). Thống kê hiển thị **Đã ôn hôm nay**, tổng thời gian của khoảng đang xem và biểu đồ cột Tuần/Tháng/Năm. Tuần bắt đầu thứ Hai; tuần/tháng có cột theo ngày, năm có 12 cột theo tháng. Có chuyển khoảng trước/sau và về hiện tại, không chuyển vượt khoảng hiện tại.
- Chỉ ghi khi có thẻ đang hiển thị, tab đang hiện, không chờ request/lỗi/chấm xong và không mở lịch sử. Không thao tác trong 60 giây (chạm/phím/cuộn) thì tạm dừng; thao tác tiếp sẽ tiếp tục. Không cộng bù khoảng máy ngủ, event loop bị treo hoặc đồng hồ quay lùi.
- Client gom khoảng ngắn, flush khoảng 10 giây hoặc khi tạm dừng/rời trang. Mỗi khoảng có UUID bất biến; retry giữ UUID. Lỗi lưu thời gian không chặn lật/chấm thẻ; hiện cảnh báo và nút thử lại. Backlog chỉ nằm trong bộ nhớ, tối đa 360 khoảng; đầy thì dừng ghi cho đến khi gửi được. Đóng/crash trình duyệt hoặc mất mạng rồi tải lại có thể mất phần chưa gửi, không giả là đã lưu.
- Server kiểm run thuộc người gọi và đã phát thẻ; tối đa 12 khoảng/request, mỗi khoảng >0 và ≤15 giây, không quá 24h cũ, không quá 5 giây tương lai. Kẹp theo lúc bắt đầu/kết thúc run và giờ server. Chỉ ghi `flashcard_study_time`, không sửa note, attempt, FSRS hoặc lịch ôn.
- Retry cùng UUID không thêm bản ghi. Tổng thời gian là **hợp các khoảng thời gian** của chính user, nên hai tab/thiết bị ôn trùng thời điểm chỉ tính một lần. Dùng `range_agg`/multirange sẵn có của [PostgreSQL](https://www.postgresql.org/docs/17/functions-aggregate.html), không tự viết phép gộp khoảng.
- API tổng hợp tại DB; chia theo lịch của IANA timezone (ngày 23/25 giờ cũng đúng), kể cả khoảng vượt nửa đêm. Ngày/tháng trống trả 0; tổng năm là tổng 12 tháng; hôm nay luôn là hôm nay ngay cả khi đang xem năm trước. Không suy ra thời gian từ `started_at/finished_at` của run cũ. `RunSummary.elapsed_seconds` hiện có vẫn là thời gian từ mở đến kết thúc run, khác với thời gian hoạt động của thống kê này.
- UI có trạng thái tải/lỗi/thử lại/trống, làm mới mỗi phút khi đang mở Thống kê, query được làm stale sau khi ghi thời gian. Chạm trên vùng biểu đồ để chọn cột; bàn phím trái/phải/Home/End cũng chọn được. Hỗ trợ 320px trở lên, không gửi request thống kê khi chỉ xem Ôn tập.
- Migration bổ sung `m20261024_010001_flashcard_study_time`, chỉ tạo bảng/index mới (PostgreSQL ≥14); deploy migration + Rust trước frontend, không cần cấu hình VPS mới. Không sửa dữ liệu lịch sử hoặc nội dung bài.

Từ 25/10, hai biểu đồ dùng Chart của shadcn-vue trên Unovis thay phần cột CSS tự dựng. Có tooltip theo theme, dùng chung component vẽ, tải thư viện riêng khi vào Thống kê; giữ API, tổng thời gian và công thức tỷ lệ.

### Hiệu suất ôn tập (24/10)

- Trong Thống kê, bên dưới thời gian ôn có **cột chồng Quên/Khó/Nhớ/Dễ**, dùng cùng Tuần/Tháng/Năm, khoảng trước/sau và timezone. Trục dọc là số **lượt chấm**, không phải số từ duy nhất; một từ được ôn nhiều lần có nhiều lượt. Màu theo controls chấm thẻ và lịch sử: đỏ/vàng/xanh lá/xanh dương, có nhãn và số đếm.
- **Tỷ lệ Nhớ / Dễ** = `(rating 3 + rating 4) / tổng rating 1..4`. Khó được thống kê riêng. Tỷ lệ cả khoảng tính từ tổng số lượt, không trung bình các tỷ lệ ngày. Chưa có lượt chấm → `remembered_rate: null`, UI hiện `—`; có lượt chấm nhưng không chọn Nhớ/Dễ → `0%`.
- Chỉ đếm attempt của user đang đăng nhập, có rating 1–4 và `reviewed_at` trong khoảng yêu cầu, không ở tương lai. Retry grade vẫn cùng row nên tính một lần; attempt mới/lật nhưng chưa chấm không tính. Dùng thời điểm chấm, không dùng thời điểm mở run hay tạo attempt. Chia theo ngày địa phương; tháng 28–31 ngày, năm 12 tháng, ngày rỗng có `[0,0,0,0]`.
- Là thống kê hoạt động cá nhân: giữ các lượt cũ, kể cả thiếu snapshot/run, từ bị sửa/gỡ hoặc nguồn mất quyền sau đó. Chỉ trả số lượng tổng hợp, không trả nội dung/thông tin nguồn ghi chú. Đây là kết quả khi ôn; thanh Ghi nhớ hiện tại vẫn dùng FSRS riêng. Không ghi DB, thay lịch, tạo migration hay backfill.
- `GET /flashcards/statistics` bổ sung `performance:{total,ratings:[again,hard,good,easy],remembered_rate:number|null,buckets:[{date,ratings:[again,hard,good,easy]}]}`. Rate là phân số 0..1. Hai biểu đồ dùng cùng một request; thêm một aggregate DB trên index `flashcard_attempts(user_id, reviewed_at DESC)`, không tải attempt body hay truy vấn vòng lặp. Client ẩn biểu đồ hiệu suất khi backend cũ chưa trả trường này.
- Chạm hoặc dùng trái/phải/Home/End để xem số lượt và tỷ lệ từng cột. Hai biểu đồ đổi cùng khoảng, response cũ trả chậm không thay dữ liệu khoảng mới. Layout hỗ trợ 320px; không thay nút lật/chấm thẻ, cấu hình, lịch sử hay thanh ghi nhớ hiện tại.

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

Mặt thẻ ôn Flashcard (24/10): từ loại hiển thị thành nhãn nền nhẹ, đậm vừa, nằm trên từ/nghĩa. Chủ đề có tiêu đề riêng và tag nền nhẹ, wrap tên dài trên mobile; cùng cách hiển thị chủ đề ở mặt đáp án. Ẩn phần thiếu dữ liệu. Chỉ thay phần từ loại/chủ đề; giữ nguyên typography nội dung chính, nút Hiện đáp án, phím Space, chấm thẻ và lịch ôn.

Tiến độ ghi nhớ trên dashboard (24/10): tổng hợp xác suất nhớ hiện tại từ `fsrs::current_retrievability`, dựa trên stability, thời gian từ lần ôn cuối và decay của mô hình cá nhân (phục hồi memory cũ theo cùng logic scheduler). Ba phần: **Còn nhớ ≈ ΣP**, **Đã quên ≈ số từ đã học − ΣP**, **Chưa ôn**; phần trăm là ΣP/tổng từ. Đây là giá trị kỳ vọng, không khẳng định một từ cụ thể đã quên. Theo bộ lọc/quyền và chiều ôn hiện tại; gom word_key, từ nhiều nguồn/nghĩa/POS/locale/chiều chỉ đếm một lần, lấy xác suất thấp nhất trong các lịch đã học. Một từ chưa có lịch đã học trong chiều đang chọn thuộc Chưa ôn; lịch chưa học của biến thể khác không làm hạ xác suất của từ đã học. Không dùng due_at để suy ra quên, không ghi DB/đổi lịch. Mục tiêu nhớ trong cấu hình vẫn là tham số lập lịch riêng. UI làm mới khi mở/focus và mỗi 5 phút khi ở dashboard.

- `GET /flashcards?session_id=&course_id=&topic=&q=&new_only=` — settings, `word_count`, `memory_progress`, `reviewable`, `new`, `due`, `learning`, thời điểm đến hạn gần nhất (đếm theo nhóm).
- `PUT /flashcards` — filters + `direction` (`vi_en|en_vi|both`) + `retention` (0,70–0,97).
- `POST /flashcards/next` — filters (+ `run_id`); trả mặt trước + UUID attempt, hoặc `card:null` kèm overview.
- `POST /flashcards/{uuid}/reveal` — đáp án (nghĩa có thể nối `; `) + khoảng ôn dự kiến cho 4 rating.
- `POST /flashcards/{uuid}/grade` — `{"rating":1..4}`.
- `POST /flashcards/runs` — tạo run từ filters → 201 summary.
- `GET /flashcards/runs/{id}`; `POST /flashcards/runs/{id}/finish` — kết thúc sớm (`completed=false`).
- `GET /flashcards/history` — phân trang mới nhất trước; lọc `session_id`, `course_id`, `topic`, `q`, `run_id`, `card_id`, `rating`, `date` hoặc `from`/`to` + `timezone`.
- `GET /flashcards/optimization` — trạng thái, số mẫu, ngưỡng, thời điểm cập nhật.
- `GET /flashcards/statistics?period=week|month|year&date=YYYY-MM-DD&timezone=Asia/Ho_Chi_Minh` — `{period,start_date,end_date,today,today_seconds,total_seconds,buckets:[{date,seconds}],as_of}`; date mặc định ngày hiện tại trong timezone (mặc định UTC), nhận năm 2000–2100. Ngày cuối là inclusive. Seconds có thể có phần thập phân; bucket của năm dùng ngày đầu tháng.
- `POST /flashcards/runs/{id}/time` — `{segments:[{id:UUID,started_at:RFC3339,ended_at:RFC3339}]}` → `{data:{recorded:true}}`; kiểm quyền run, validation cả batch trước khi ghi, idempotent, không trả dữ liệu run người khác.

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
- GET trả trang 100 row. Chế độ cặp âm chỉ tải pairs; Tất cả từ cần cả words và pairs để giữ các từ nhập qua cặp âm. Tìm kiếm local vẫn tải các trang của nguồn đang cần (chưa phù hợp kho cực lớn). Preview import không tải lại toàn bộ kho, chỉ lookup các khóa trong payload.
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

- Hiệu năng R6: tên chip ẩn/ghim dùng catalog/place-list sẵn có; chỉ bulk các ID thiếu (1–100/request) khi navigation cần, không đọc từng detail hoặc tải từ popover đóng. `places/bulk` và `courses/bulk` giữ đúng owner/parent đang hoạt động. Learn desktop 7 → 6, note desktop 11 → 10; mở drawer mobile 5 → 4 sau R5. Unit kiểm tra metadata ngoài trang đầu, bỏ ID thiếu và không tải khi picker đóng.

- Hiệu năng R7: tab Lịch sử chỉ tải danh sách lịch sử, không overview/recent dashboard (3 → 1 API sau R2, hai kích thước). Quay lại Ôn tập bật lại query đúng lúc, dùng cache còn mới hoặc tải lại nếu cũ. Browser kiểm tra cả lịch sử cold và quay lại khi cache hết hạn.

- Hiệu năng R8: trang IPA mặc định chỉ tải pairs (2 → 1 API sau R2; 4 → 1 từ baseline gốc). Chuyển Tất cả từ mới tải words; vẫn giữ từ phát sinh từ pairs. Hai endpoint `POST /ipa-vocabulary/duplicates` và `/ipa-pairs/duplicates` là lookup read-only, dùng chung validator/canonical keys với import, SELECT theo user/locale/kind và khóa payload, có kiểm tra trùng nội bộ. Import thật vẫn kiểm tra trong transaction và giữ idempotency. Browser thử kho ba trang xác nhận preview chỉ gọi một lookup; Rust thử từ ở trang hai, đảo cặp, trùng payload và user khác.

- Xác minh R0: coordinator run [39037695883](https://github.com/thienbd203/pathnote/actions/runs/39037695883) đã qua check, build, browser contract đúng cặp SHA và deploy. Bản tối ưu R1–R8 được kiểm tra local với cùng bộ test, cần chạy CI riêng sau khi push cặp SHA mới.


### Số đo tổng thể sau R1–R8

Bộ fixture cố định: 3 nơi, 6 khóa, 12 buổi, 5 buổi gần đây và note 24 bài tập;
cache mới khi mở màn, không tính auth refresh. Đây là số request của kịch bản,
không phải cam kết thời gian tải trên mạng production.

| Màn/thao tác | Desktop trước → sau | Mobile trước → sau |
| --- | --- | --- |
| Sao chép prompt AI (cache cũ) | 24 → 1 | 24 → 1 |
| Ghi chép / Learn | 12 → 6 | 9 → 2 |
| Thư viện | 7 → 5 | 12 → 8 |
| Note | 11 → 10 | 7 → 5 |
| Lịch sử flashcard | 5 → 1 | 5 → 1 |
| Mở lượt ôn | 6 → 2 | 6 → 2 |
| Chấm rồi chuyển thẻ | 5 → 2 | 5 → 2 |
| IPA mặc định | 4 → 1 | 4 → 1 |
| Cài đặt | 3 → 1 | 3 → 1 |

Kiểm tra bản ghép: 206 test frontend; 63 ca Playwright qua cả macOS và Linux,
8 ca browser với Rust/PostgreSQL thật; 114 test Rust (109 API), fmt và Clippy
không warning. Ảnh chuẩn giữ nguyên. Một ca Playwright chỉ dành cho mobile được
skip có chủ đích trên desktop. Bản tối ưu cần xác minh CI sau push.

## Triển khai API blue–green

- CI hỗ trợ `DEPLOY_STRATEGY=blue-green` sau khi VPS cài hai systemd slot và helper Nginx root-owned. Mặc định vẫn in-place cho VPS chưa chuyển đổi; từ chối in-place khi đã có slots.
- Slot blue/green lần lượt 3000/3001, địa chỉ nội bộ ổn định 3010 cho collaboration. Giữ chung PostgreSQL, JWT và media. Chỉ chuyển traffic sau health đúng commit SHA; xác minh nội bộ và public trước đổi current, drain rồi stop slot cũ.
- Lỗi target giữ traffic cũ; lỗi sau switch tự rollback, nếu rollback không xác minh được thì giữ cả hai process. Giữ releases của cả hai slot; drain timeout giữ process cũ. Migration phải tương thích ngược, cờ breaking bị chặn, không tự rollback database.
- Dùng graceful shutdown Axum hiện có, systemd và Nginx reload; helper không nhận đường dẫn tùy ý. Giới hạn: một VPS, không HA database/máy chủ, không thay quy trình collaboration hay chính sách asset frontend.
- Lượt production đầu [39366022269](https://github.com/thienbd203/pathnote/actions/runs/39366022269) đã qua và chuyển green → blue với backend `a51fdc0`. Theo dõi HTTPS health trong khoảng 159 giây: 71 mẫu bản cũ + 5 mẫu bản mới, không có lỗi; đây là lấy mẫu endpoint health, chưa phải kiểm chứng mọi request hay bài test rollback production.

Code: `.github/deploy.sh`, `deploy/{blue-green.py,upstream.py,pathnote-rust@.service}`. Hướng dẫn chuyển VPS, rollback và maintenance: `deploy/README.md` mục 7.

### Thứ tự phát hành ba dịch vụ (21/10/2026)

Rust coordinator đóng gói ba revision bất biến; build/test song song, kích hoạt Rust → collaboration → frontend. Push repo web/collaboration chỉ chạy CI. Collaboration giữ nguyên socket nếu `/up` đã đúng SHA, chỉ restart khi thay revision; lỗi deploy/health chặn frontend. Frontend vẫn kiểm đúng SHA của cả hai dịch vụ trước kích hoạt. Hướng dẫn variables/quyền SSH và rollback ở `deploy/README.md` mục 3–4.

### D1/D3: tab cũ và readiness realtime

Đã tái hiện schema Tiptap cũ bỏ ipaUK/ipaUS khi serialize và backend cũ nhận bản thiếu. Schema ghi chú hiện tại 4; version là capability của code, không copy từ note. Writer cũ bị 409 client_outdated trước khi thay dữ liệu; note schema 3 đã có hai IPA cũng được bảo vệ. Bridge khai báo version khi authorize/bootstrap/store và chặn client WebSocket thiếu/version cũ trước khi sync Y.Doc. Frontend phân biệt lỗi này với conflict revision, giữ bản đang viết, khóa ghi đè/retry tự động, có tải JSON bản nháp và tải lại; không tự reload mất nội dung. Tab chạy JS trước D1 không thể có UI mới từ server: việc bảo vệ là chặn ghi; người dùng cần reload thủ công.

Grace đúng một release schema 4 chỉ cho payload thiếu/null version trên note legacy chưa có attr hai giọng, chưa được nâng version; không áp dụng realtime client. Schema 5 tự tắt grace, cần bỏ nhánh ở release tiếp theo (xem architecture).

Readiness collaboration `/up` kiểm Rust qua URL cấu hình (timeout 2s, cache 10s). Rust/DB lỗi làm HTTP 503 và rust:error, coordinator không cho frontend mới kích hoạt. Mẫu production/startup dùng listener 3010; local/in-place phải override 3000. `.env` production thực tế do người vận hành xác minh theo README collaboration; Codex chưa đọc hay sửa VPS trong lần fix này.

### Rà soát Rust C1 (26/10)

- C1.1: optimizer ghi `checked_at` trước khi đọc lịch sử, kể cả thiếu mẫu/không đủ mẫu mới/lỗi đọc. Tick sau trong một giờ không tải lại lịch sử. Projection lịch sử chỉ lấy cột phục vụ FSRS, bỏ JSON snapshot/parameters. Thêm `evaluated_at` (migration additive, backfill từ checked_at cho model đã đánh giá) để mốc 30 ngày không bị các lần kiểm tra định kỳ đẩy lùi mãi. Không đổi lịch thẻ hoặc tham số khi chỉ bỏ qua. Test metric SQL xác nhận hai nhánh bỏ qua và vẫn đánh giá lại sau 30 ngày.

- C1.2: trạng thái optimizer dùng một aggregate SQL, không tải JSON/lịch sử vào Rust và không còn giới hạn 50.000 lượt của bộ fitting. Chỉ đếm chuỗi bắt đầu version 0, đủ thứ tự, delta hợp lệ; mẫu dùng tối đa 256 lượt/chuỗi. Test chuỗi lỗi, reset, chuỗi dài và hơn 50.000 lượt, tách dữ liệu từng user.

- C1.3: overview chỉ tải cột memory/lịch sử cần để tính FSRS; replay từ 32 chuỗi stale hoặc trên 512 lượt được chuyển sang blocking pool. Lịch sử thiếu dùng stability đã lưu, state không hợp lệ được bỏ qua, không làm hỏng toàn bộ overview. Giữ GET read-only để tránh ghi replay đè lên lượt chấm đồng thời; lịch ôn không đổi. Test fallback qua API và 64 thẻ có một state lỗi.

- C1.4: ghi chú mới (trống, template, sao chép cấu trúc) đều khai báo schema hiện tại 4; writer thiếu version bị 409 client_outdated. Chỉ note legacy đã lưu mới dùng hằng legacy 3 cho grace D1. AI import/IPA validation dùng version hiện tại. Test tạo cả ba loại và chặn writer cũ, giữ test grace cho dữ liệu cũ.

- C1.5: tìm buổi theo ngày chọn midnight sớm nhất khi DST lặp giờ; nếu nhảy qua midnight thì chọn giây hợp lệ đầu ngày. Ngày bị bỏ toàn bộ không thêm điều kiện ngày, vẫn tìm theo tiêu đề, không trả 422. Test Santiago (gap), Havana (overlap) và Apia (ngày bị bỏ).

- C1.6: lưu vị trí nghe dùng một upsert có điều kiện quyền/file/session/cha active trong chính SQL, không giữ khóa place. Test giữ khóa place nhưng insert và update vị trí vẫn hoàn tất; sai session, nguồn đã xóa và share bị thu hồi bị 404, không ghi đè vị trí cũ. Không đổi note revision.

- C1.7: AI import và tạo bài tập dùng chung helper insert_many trả ID; validate/serialize vẫn trước khóa, insert/receipt vẫn cùng transaction. Test metric xác nhận 100 câu chỉ một INSERT, giữ thứ tự placeholder, retry không tạo trùng và batch rỗng không lỗi.

- C1.8: multipart chọn cap trước khi đọc chunk: ảnh 5 MiB, MP3/M4A/MP4 (không phân biệt hoa/thường) 100 MiB, các đuôi còn lại 10 MiB. Vẫn kiểm bytes/đuôi thực trong blocking pool; đuôi recording không bỏ qua kiểm định dạng. Test response chặn trước inspect và recording trên 10 MiB vẫn nhận. Giữ buffering hiện tại (tối đa cap) vì chuyển temp-file cần thay cả đường inspect/storage và quản lý lease/cleanup; mục này áp dụng bản sửa tối thiểu.

- C1.9: bridge fetch/store chỉ SELECT note một lần/request (bao gồm Yjs); fetch dựng note/state từ cùng snapshot, store dùng model INSERT/UPDATE trả về. Giữ kiểm quyền, schema, reference, revision và retry idempotent. Test metric SQL với state 2 MiB xác nhận một SELECT cho fetch, lưu, retry và conflict; conflict không thay content/revision/state.

### Rà soát Rust C2 (26/10)

- C2.1: REST và bridge dùng chung persist trong transaction; cùng điều kiện tồn tại dựa saved_at (kể cả note legacy revision 0), cùng mốc saved_at/updated_at. Kiểm quyền/schema/conflict/idempotence vẫn ở caller; đồng bộ projection và commit vẫn nguyên tử. Test tạo/cập nhật cả hai đường, note legacy và timestamp bằng nhau.

- C2.2: xóa một place/course dùng chung cascade của bulk; giữ kiểm quyền, transaction và timestamp chung, không đổi deleted_at con đã xóa. Bulk lấy toàn bộ khóa place bằng một SELECT FOR UPDATE theo ID tăng dần, kiểm lại quyền sau khóa. Test SQL chỉ một câu khóa, cascade đơn/bulk và concurrent bulk hiện có.

- C2.3: Google dùng read lock cho metadata còn TTL; chỉ miss/expiry mới lấy write lock và kiểm lại cache sau khi chờ, tránh discovery trùng. HTTP timeout, issuer, OAuth PKCE/nonce/state và xác minh token giữ nguyên. Test hai authorization đồng thời khi một reader khác giữ cache, không gọi Google thật; các test OAuth hiện có tiếp tục chạy.

- C2.4: next lấy title/owner cùng truy vấn chọn thẻ có scope quyền; bỏ notes::access tải lại session/place sau INSERT. Load meanings/model trước tạo ticket để lỗi đọc không để ticket mồ côi. Giữ một aggregate counts vì frontend đang dùng result.overview cập nhật số thẻ sau mỗi lượt; không đổi contract hoặc tính memory_progress trên next. Test metric 5 SELECT cho lượt global một chiều, source và overview vẫn đúng; các test quyền/share/revocation/lịch thẻ giữ nguyên.

- C2.5: tạo lượt ôn dùng partial model cho thẻ, distractor, nguồn và owner; typing không tải pool distractor. Choice giữ random sample giới hạn 1.000 dòng, chỉ tải 6 cột cho distractor, chuẩn hóa nghĩa một lần thay vì trong vòng lặp; key giữ nguyên. Tạo review trong snapshot RepeatableRead, bỏ authorize/current đọc lại sau INSERT; show/answer vẫn kiểm quyền và nội dung mới nhất. Test projection SQL, số pool, không SELECT sau INSERT, loại distractor cùng nghĩa khác hoa/thường/khoảng trắng; giữ các test UK, retry, revoke và progress.

- C2.6: Question dùng một struct chứa 5 field chung và Kind flatten; validate/public/grade không còn match lặp để lấy cùng field. JSON giữ nguyên đủ 7 loại; strict wire adapter chỉ dùng lúc deserialize vì Serde không hỗ trợ flatten + deny_unknown_fields. Giữ từ chối field lạ/trùng, items sai loại/null/thiếu, không lộ answers/explanation. Test round-trip đủ 7 loại và strict input; toàn bộ test chấm bài tập hiện có.

- C2.7: reveal, grade thiếu snapshot và lịch sử legacy dùng chung factory snapshot với locale/IPA/ví dụ UK và nghĩa gộp theo identity. Lịch sử gom nghĩa cả trang bằng một query, không N+1; khi source đã inactive và không còn cluster active thì giữ nghĩa nguồn chưa thay đổi. Snapshot đã lưu giữ nguyên, gồm accent cũ; content_key khác vẫn không dựng fallback. Test cả ba đường, nhiều log chỉ một query nghĩa, UK word/example, nghĩa gộp và không đổi FSRS.

### Rà soát Rust C3 (26/10)

- C3.1: ghi rõ ngoại lệ A.3 tại các handler bulk/IPA/flashcard/review/library: giữ thông báo theo field nghiệp vụ để tương thích giao diện, không thay JSON/status. Các extractor thông thường tiếp tục đi qua AppJson/AppQuery/AppPath.

- C3.2: cấu hình run được deserialize bằng Configuration và kiểm retention, không mặc định khi direction/retention đã lưu bị hỏng. RunSummary filters/settings, HistoryEntry.card, review Card.source và filters dùng DTO có kiểu; shape hợp lệ giữ nguyên, không thay snapshot accent cũ. Test cấu hình lỗi trả 500 trước INSERT ticket, khôi phục config đọc được; toàn bộ test JSON/history/review hiện có.

- C3.3: kiểm finite và miền retention/memory trước FSRS, kiểm output trước clamp/cast; ngày đến hạn dùng checked_add. NaN/Infinity không thể thành thẻ đến hạn ngay. Test interval/retention không hữu hạn, clamp hữu hạn giữ nguyên, API reveal với stability NaN trả 500 mà không đổi ticket/lịch.
