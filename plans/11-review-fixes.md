# Sửa lỗi sau review toàn project — 16/10/2026

> **Trạng thái 17/10/2026:** đã hoàn tất phần sửa code và tài liệu của P0/P1/P2, mỗi mục một commit theo repo. Xem [báo cáo và commit](11-review-fixes-report.md). Các bước kiểm tra/cấu hình production dành cho người vận hành vẫn cần thực hiện trước khi deploy.

Danh sách này đến từ đợt review lại sau khi hoàn tất `10-architecture.md` và commit gộp flashcard `d83be81`. Các mục P0 và P1 đã được xác minh lại trực tiếp trên code. Mục P2 lấy từ báo cáo review và đã đọc lại vị trí code được trỏ tới.

## Quy tắc chung

- **Thứ tự:** làm từ trên xuống. P0 phải xong trước khi deploy production lần tới.
- **Commit:** mỗi mục là một commit, trong đúng repo của mục đó. Không gộp nhiều mục vào một commit.
- **Đường dẫn:** tính từ thư mục `pathnote/`.
- **Kiểm tra sau mỗi commit:**
  - Rust: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
  - Web: chạy bằng Node 22 theo `.nvmrc`, gồm `npm test`, `npm run build` và `npm run format:check` (nếu chưa có script này thì chạy `npx prettier --check src`).
  - Collaboration: `npm test`.
  - Pronunciation: chạy test trong `tests/`.
- **Thay đổi contract API:** cập nhật bảng B.6 trong `10-architecture.md`.
- **Báo cáo:** khi xong, ghi `plans/11-review-fixes-report.md` gồm bảng mục → commit, những gì đã kiểm tra, và những gì chưa kiểm tra được.

---

## P0 — An toàn deploy (pathnote-rust)

### P0.1 Backup database trước khi chạy migration

**Lỗi:** `.github/deploy.sh:78` chạy `migration up` mà không backup trước. Migration `m20261016_020001` xóa các dòng `flashcard_memory` trùng và có `down()` trả lỗi, nên nếu release lỗi sau migration thì không còn bản dữ liệu nào để khôi phục.

**Sửa:**
- Ngay trước dòng `migration up`, chạy `pg_dump --format=custom` (dùng `DATABASE_URL` đọc từ `$SHARED/.env`) vào `$DEPLOY_PATH/backups/<RELEASE>.dump`.
- File backup: quyền `600`, owner root. Chỉ giữ 5 bản mới nhất.
- User deploy hiện không có quyền root cho backup. Cài helper cố định thuộc root, chỉ cho phép tên release hợp lệ; helper chạy `pg_dump` bằng user ứng dụng và ghi vào thư mục backup thuộc root. Không cấp sudo cho script do deploy upload hoặc cho shell tùy ý.
- Đọc DATABASE_URL dạng một dòng, hỗ trợ giá trị có/không có dấu nháy; cấu hình không hỗ trợ phải báo lỗi rõ ràng, không tự diễn giải bằng shell.
- `pg_dump` lỗi, hoặc file dump rỗng, thì deploy dừng và không chạy migration.
- Thêm `command -v pg_dump` vào phần kiểm tra đầu script.
- Đọc `DATABASE_URL` mà không source cả file `.env`, ví dụ `grep -m1 '^DATABASE_URL=' | cut -d= -f2-`. Không in URL ra log.

**Cập nhật tài liệu:**
- `deploy/README.md`: cách cài `postgresql-client` đúng major version với server.
- `deploy/coordinated-release.md`: lệnh restore bằng `pg_restore --clean --if-exists`.

**Kiểm tra:** thêm test trong `deploy/tests/` nếu kiểu test hiện có cho phép. Nếu không, ghi rõ trong báo cáo là chưa test được trên VPS.

### P0.2 Chế độ migration không tương thích ngược

**Lỗi:** `m20261016_020001` thêm hai cột `flashcard_attempts.content_key` và `memory_card_id` kiểu `NOT NULL`, không có default. Migration chạy khi binary cũ vẫn đang phục vụ, vì dòng 78 nằm trước bước switch ở dòng 81. Hậu quả:
- Trong khoảng thời gian đó, mọi lượt chấm flashcard bằng binary cũ đều lỗi.
- Nếu health check thất bại, `on_exit` sẽ chuyển về binary cũ, nhưng binary cũ không chạy được với schema mới.

**Sửa:**
- Thêm khóa `"breaking_migration": true|false` vào `.github/release.json`. Workflow truyền giá trị này cho `deploy.sh` như một tham số mới.
- Khi giá trị là `true`, thứ tự deploy là: `systemctl stop` → backup (P0.1) → `migration up` → switch → start → health check.
- Ở chế độ này, nếu lỗi xảy ra sau khi migration đã chạy:
  - **Không** tự chuyển về release cũ.
  - Để service dừng.
  - In ra đường dẫn file dump và lệnh restore.
- Nếu đã dừng service nhưng backup lỗi trước khi bắt đầu migration: khởi động lại bản cũ nếu có. Đánh dấu migration đã bắt đầu **trước** khi gọi lệnh, vì migration có thể thay đổi một phần rồi thất bại.
- Khi giá trị là `false`, giữ nguyên luồng hiện tại (vẫn backup trước migration).
- Đặt `true` cho release đầu tiên chứa `m20261016_*`.
- `deploy/coordinated-release.md` ghi quy tắc: mọi migration thêm cột `NOT NULL` không có default, hoặc xóa/đổi dữ liệu, đều phải bật cờ này.

### P0.3 Checklist trước release đầu tiên có `m20261016_*`

Thêm vào `deploy/coordinated-release.md`. Đây là việc **người vận hành** làm, Codex chỉ viết tài liệu:

- Chạy `SELECT lower('É') = 'é';` trên DB production. Kết quả phải là `true`; nếu không thì dừng deploy.
  - Lý do: hash nhóm từ trong migration dùng `lower()` của Postgres, còn Rust dùng `to_lowercase`. Trên cluster có locale `C`, hai bên sẽ lệch nhau với chữ có dấu.
- Xác nhận đã có file backup từ P0.1.

---

## P1 — Lỗi người dùng thấy được hoặc hiệu năng đường nóng

### P1.1 [rust] Index cho truy vấn gom nghĩa flashcard

**Lỗi:** `server/src/modules/vocabulary/flashcards/queue.rs:41-63` (`meanings()`) lọc `vocabulary_cards` theo `word_key, locale, part_of_speech`, nhưng không có index cho các cột này. Truy vấn chạy ở mỗi lần `next` và `reveal`.

**Sửa:**
- Tạo migration mới với index `vocabulary_cards (word_key, locale, part_of_speech)`.
- Cùng migration: xóa index `flashcard_memory_identity` (tạo ở `m20261016_010001`, dòng 37), vì nó trùng phần đầu của index `flashcard_due (user_id, direction, due_at)`.

### P1.2 [rust + web] Timeout dịch đang đua với timeout của route

**Lỗi:**
- Upstream dịch có timeout 30 giây (`enrichment/client.rs:143`), bằng đúng timeout của route (`app.rs`). Route sẽ luôn hết giờ trước, nên người dùng nhận 408 thay vì 503 kèm hướng dẫn tự nhập.
- Ngoài ra, timeout phía server đang trả 408, là mã lỗi phía client.

**Sửa:**
- Đặt timeout dịch còn 20 giây.
- Đổi `AppError::RequestTimeout` sang trả **504**. Giữ nguyên message.
- Web, `src/lib/api/enrichment.ts`: timeout của translation đổi từ 35 giây xuống 28 giây (vẫn dài hơn upstream, ngắn hơn route).
- Ghi vào bảng B.6: route timeout trả 504 thay vì 408.

### P1.3 [web] Buổi học hoặc khóa học đã xóa vẫn hiện trong "gần đây" và "Tiếp tục"

**Lỗi:**
- Khi xóa, `useSessionMutation` (`src/lib/api/sessions.ts:166-191`) chỉ invalidate query. Lúc refetch nhận 404, query vẫn giữ `data` cũ.
- `LearnView.vue:36-52` vì thế vẫn hiện buổi đã xóa.
- `workspace.recent` và localStorage không bao giờ được dọn.

**Sửa:**
- Trong mutation xóa đơn và xóa hàng loạt của sessions: gọi `qc.removeQueries({ queryKey: sessionKeys.detail(id) })` cho từng ID, và xóa các ID đó khỏi `recent` (gồm cả localStorage).
- Mutation xóa course và place: làm tương tự cho các key detail của chúng.
- Khi một ID trong `recent` trả 404, cũng loại ID đó khỏi `recent`.
- Thêm test: xóa session thì detail query bị loại khỏi cache và ID không còn trong `recent`.

### P1.4 [web] Chip khóa học trên sidebar không cập nhật

**Lỗi:** `src/composables/useWorkspaceData.ts:39-45` tạo infinite query `catalog` với `enabled: false`. Khi `invalidateQueries(['courses'])`, query này chỉ bị đánh dấu stale, không được tải lại. Hậu quả: đổi tên hoặc xóa khóa học thì chip vẫn giữ tên cũ hoặc trỏ tới ID đã chết.

**Sửa:** `refetchType: 'all'` vẫn bỏ qua query đang có observer `enabled: false` (đã kiểm tra bằng QueryObserver của phiên bản đang cài). Chuyển catalog sang query có điều kiện enabled theo đăng nhập/nhu cầu hiển thị, hoặc chủ động gọi `fetchInfiniteQuery` sau invalidation. Không chỉ thêm refetchType. Giữ kiểm tra generation/user khi đổi tài khoản; kiểm tra watcher ở `StudyTimeline.vue:170` để không tải trùng.

**Test:** đổi tên khóa học thì catalog được fetch lại.

### P1.5 [collaboration] Cấu hình mặc định vẫn trỏ về Laravel

**Lỗi:**
- `server/start.mjs:13-14` và `.env.example:2-3` mặc định là `http://127.0.0.1:8001` với prefix `/api` (của Laravel).
- Template `.env` trong README thiếu `COLLABORATION_API_PREFIX`.
- Nếu prefix sai, `collaboration.mjs:95,114` dùng schema không phải Rust và mất attr `aiImportId` khi lưu.

**Sửa:**
- Mặc định đổi sang Rust: URL `http://127.0.0.1:3000`, prefix `/api/v1`. Khi kiểm tra, đọc `pathnote-rust/.env.example` để lấy đúng port và prefix.
- Cập nhật `.env.example` và template trong README.
- Thêm cảnh báo khi khởi động nếu prefix không phải `/api/v1`.

### P1.6 [collaboration, pronunciation] Không pin host key SSH

**Lỗi:** workflow deploy của collaboration (`.github/workflows/deploy.yml:70`) và pronunciation (`.github/workflows/deploy.yml:66`) dựng `known_hosts` bằng `ssh-keyscan` ngay lúc deploy. Cách này tin bất kỳ host key nào được trả về, nên mất tác dụng chống giả mạo server.

**Sửa:** dùng secret `SSH_KNOWN_HOSTS` như `pathnote-rust/.github/workflows/deploy.yml:208-216`. Cập nhật README phần secret của từng repo.

### P1.7 [rust + collaboration] Đưa collaboration vào release phối hợp

**Lỗi:** collaboration deploy độc lập. Nếu frontend thêm node Tiptap trước khi collaboration có node đó, `node.check()` (`collaboration.mjs:36-37`) sẽ lỗi mỗi lần lưu, document kẹt ở trạng thái `failed` và mất bản sửa khi restart.

**Sửa:**
- Thêm `collaboration_sha` vào `.github/release.json`.
- Trước khi kích hoạt frontend, workflow Rust kiểm tra rằng `/up` của collaboration (qua HTTPS) trả đúng SHA này. Không khớp thì dừng trước bước kích hoạt frontend.
- `deploy/coordinated-release.md` ghi thứ tự khi đổi schema: collaboration → backend → frontend.

---

## P2 — Mức thấp

### Web

1. **Số buổi học bị mất**
   - `LibrarySessions.vue:84` dùng `watch` không có `immediate`.
   - `LibraryView.vue:118-125` reset `summaries` mỗi khi `fullPath` đổi.
   - Sửa: đọc `session_count` trực tiếp từ `useCourseStatistics` (library-stats), bỏ cơ chế emit `loaded`/`summary` để chuyển số đếm.
2. **Nguồn "Như buổi gần nhất" lấy nhầm của khóa trước**
   - `WorkspaceCreateDialog.vue:205-216` và `:65`: `keepPreviousData` vẫn giữ buổi học của khóa A sau khi đã chuyển sang khóa B, dẫn tới lỗi 422.
   - Sửa: đặt `placeholderData: undefined` cho query này, giống `priorQuery` trong `FlashcardHistory.vue`.
3. **Thiếu invalidate**
   - `invalidateSavedNote` (`src/lib/api/notes.ts:121-130`): thêm `['library-stats']`.
   - `useImportAIStudy` (`src/lib/api/ai-study.ts:30`): thêm `['exercises']` và `['library-stats']`.
   - Cập nhật bảng invalidate trong `docs/architecture.md`.
4. **Lỗi hành động lan sang buổi học khác:** `SessionView.vue:68-91`. Khi `sessionId` đổi, tăng `version` và xóa `actionError`.
5. **Polling không bao giờ chạy:** `refetchInterval` theo `meta.syncing` trong vocabulary là code chết, vì backend luôn trả `syncing: false` (`library/service.rs:142`). Xóa phần polling. Nếu backend vẫn cần field `syncing` thì giữ field đó.

### Rust

6. **404/405 thiếu header chung:** `app.rs:78-81`. Đặt `.fallback()` và `.method_not_allowed_fallback()` trước `.layer(browser_request)`. Test: 404 có `Cache-Control: no-store`.
7. **Request ID chưa vào log**
   - Đưa `x-request-id` vào span của `TraceLayer` (`app.rs:84`).
   - Thêm `x-request-id` vào `expose_headers` của CORS.
8. **Nén cả file tải về:** `CompressionLayer` đang nén file docx, zip, pdf. Gắn compression cho router API trừ route media, hoặc dùng predicate chỉ nén `application/json` và `text/*`.
9. **Tổng kết lượt ôn đếm sai:** `flashcards/service/runs.rs:38` đếm `DISTINCT (card_id, direction)`. Sau khi gộp nhóm từ, phải đếm `(study_key, direction)`. Thêm test: một từ ở hai bài, chấm Again một lần, chỉ được đếm 1.
10. **Lệch chuẩn còn sót**
    - `auth/handlers.rs:113` (`me`): chuyển SQL vào service.
    - `vocabulary/reviews/answer.rs:32-63`: khóa nhiều place theo thứ tự ID tăng dần là hợp lý. Thêm vào A.1 của `10-architecture.md` như một ngoại lệ có chủ đích, kèm comment một dòng tại code.
11. **Giới hạn kích thước batch IPA chưa tính phần bọc:** `enrichment/service.rs:28-31` chỉ đo mảng `texts`. Phải đo cả body `{"texts":…,"locale":…}` so với giới hạn 65536 byte của `pathnote-pronunciation/server.py:40`, hoặc giảm ngưỡng phía Rust xuống 60000.

### Collaboration

12. **Reauthorize ngắt kết nối khi API chỉ lỗi tạm:** `collaboration.mjs:187` đóng socket với mọi lỗi. Chỉ đóng khi Rust trả 401, 403 hoặc 404. Với lỗi 5xx hoặc timeout, giữ kết nối và thử lại ở chu kỳ sau. Thêm test cho cả hai trường hợp.
13. **Mất bản lưu lỗi khi tắt service:** khi nhận SIGTERM (`start.mjs:22`), thử lưu lại các document đang ở trạng thái `failed` một lần (timeout tổng 10 giây) trước khi thoát.
14. **README còn nói Laravel:** viết lại các phần Laravel/`pathnote-api`/port 8001/PHP-FPM (dòng 3, 16, 24, 100-109, 133) theo backend Rust.
15. **`.gitignore`:** thêm `.keys`.

### Pronunciation

16. **Không có timeout đọc socket:** `server.py:39-42`. Đặt `timeout = 10` trên lớp handler để client chậm không giữ thread mãi. Thêm `TasksMax=64` vào unit systemd trong `deploy/`.
17. **README còn nói Laravel:** viết lại các phần Laravel (dòng 3, 41-51, 108-114). Rust là bên gọi service này.
18. **Dependency dịch chưa khóa đủ:** `requirements-translation.txt` chỉ pin trực tiếp `libretranslate`. Tạo lock đầy đủ kèm hash, ví dụ bằng `pip-compile --generate-hashes`, và cài với `--require-hashes`. Cập nhật key hash venv trong `deploy.sh:52` theo file lock.
19. **`.gitignore`:** thêm `.env.rust`.

### Web (repo)

20. **`.gitignore`:** thêm `.env` (hiện chỉ có `*.local`).

---

## Không làm

- Snapshot nghĩa trong lịch sử ôn vẫn giữ nghĩa từ bài đã mất quyền. Đây là nội dung người dùng đã xem, nên giữ như thiết kế.
- Race khi ẩn hai chip liên tiếp trong `useNavigationPreferences.hide`.
- Overview flashcard refetch sau mỗi lần chấm. Chỉ tối ưu nếu đo thấy chậm.
- Index trên jsonb `identity` của `flashcard_memory`.
