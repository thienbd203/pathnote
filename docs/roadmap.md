# Việc còn mở

Cập nhật 18/10/2026. Đây là nơi duy nhất ghi việc chưa làm của toàn project.

Khi bắt đầu một mục, ghi kế hoạch ngắn ngay dưới mục đó; khi xong, cập nhật `docs/features.md` (hoặc `docs/architecture.md`) và xóa mục khỏi đây.

**Xác minh bản tối ưu R1–R8 trên CI**

R1–R8 đã triển khai, đo lại bằng Playwright và hạ ngân sách tương ứng. Hành vi hiện tại ở `docs/features.md`; ngân sách ở `pathnote-web/e2e/request-budgets.ts`.

- Sau khi push frontend và coordinator, xác minh browser gate của đúng cặp SHA mới xanh. Bộ test/gate gốc R0 đã qua trên GitHub ở run `39037695883`; không còn chờ xác minh R0.
- Không tăng ngân sách hoặc cập nhật ảnh chỉ để làm CI xanh. Khi bản tối ưu đã qua CI, xóa mục này.

**An toàn triển khai và rollback** (review 21/10/2026, D1 đã sửa/test ở local; còn các bước dưới đây)

Thứ tự kích hoạt Rust → collaboration → frontend là đúng: backend nhận dữ liệu mới trước khi có client ghi nó. Rollback tự động trong lúc deploy cũng an toàn vì xảy ra trước khi frontend mới được kích hoạt. Rủi ro nằm ở ba chỗ: tab cũ, rollback thủ công, và cấu hình collaboration chưa được kiểm tra. Các mục dưới đây xử lý ba rủi ro đó.

- **D2 — Rollback thủ công qua thay đổi document ghi chú làm lưu ghi chú lỗi** (`pathnote-rust`; ưu tiên cao)
  - **Đã xác minh:** `notes/document/attributes.rs` dùng `deny_unknown_fields`. Backend trước tính năng UK/US từ chối mọi ghi chú đã có `ipaUK/ipaUS`. Hậu quả: autosave lỗi, collaboration không persist được.
  - **Việc 1:** thêm trường `rollback_floor` (backend SHA) vào `.github/release.json`. Đây là SHA thấp nhất được phép quay về. `deploy/release.py` kiểm tra trường này có mặt và đúng định dạng. Khi một release đổi document hoặc contract theo hướng bản cũ không đọc được, đặt `rollback_floor` bằng SHA của chính release đó.
  - **Việc 2:** ghi floor đang áp dụng vào `/opt/pathnote-rust/rollback-floor` khi deploy thành công.
  - **Việc 3:** mục rollback thủ công trong `deploy/README.md` 7.6 bắt đầu bằng bước so sánh SHA của slot đích với floor. Nếu slot đích cũ hơn floor thì **không rollback**, chỉ sửa và phát hành bản mới. Thêm một script nhỏ `deploy/rollback-check.py` trả lỗi khi vi phạm floor, và gọi nó trong các lệnh mẫu.
  - **Việc 4:** đặt floor của release hiện tại bằng SHA của release UK/US (`f18d6c1` hoặc commit chứa thay đổi attributes; Codex xác định lại).
  - **Việc 5:** quy tắc trong `docs/architecture.md`: thay đổi document/schema nên tách làm hai release khi có thể. Release đầu để backend chấp nhận trường mới, release sau để client bắt đầu ghi trường đó. Khi đó floor chỉ cần nâng ở release thứ hai.

- **D3 — xác minh cấu hình collaboration trên VPS** (code đã xong, còn bước người vận hành)
  - `/up` đã kiểm Rust/database, timeout 2 giây, cache 10 giây, Rust lỗi trả 503/rust:error. Deploy gate yêu cầu rust:ok; mẫu production và startup dùng listener 3010. Test dead port/hung HTTP/cache/recovery đã qua.
  - **Còn mở — người vận hành chạy:** đọc `COLLABORATION_API_URL` trong `/var/www/pathnote-collaboration/shared/.env`; với blue–green phải là `http://127.0.0.1:3010`. Nếu đang là 3000/3001 thì sửa và restart collaboration; lệnh đầy đủ trong README collaboration mục D3. Codex chưa đọc/sửa VPS trong lần fix này.
  - Sau deploy D1/D3, xác nhận local/public `/up` có status:ok và rust:ok, rồi đóng mục này.

- **D4 — Deploy collaboration không rollback khi bị hủy giữa chừng** (`pathnote-collaboration/.github/deploy.sh`)
  - **Hiện trạng:** script chỉ có `trap rollback ERR`, chưa có `set -E`, và không trap `HUP INT TERM`. Hủy job GitHub (SSH ngắt) sau khi đổi symlink thì không rollback.
  - **Việc:** thêm `set -E` và trap tín hiệu giống `pathnote-rust/.github/deploy.sh`.
  - **Test:** gửi SIGTERM sau bước đổi symlink, xác nhận release cũ được khôi phục. Viết test theo kiểu `deploy/tests` nếu có, không thì ghi rõ là chưa test.

- **D5 — Lệnh rollback thủ công trỏ tới file trong `/tmp`** (`deploy/README.md` 7.6 và chỗ dòng 573)
  - **Hiện trạng:** các lệnh mẫu dùng `/tmp/pathnote-bluegreen/healthcheck.py`, file không chắc còn tồn tại.
  - **Việc:** đổi sang `"$(readlink -f /opt/pathnote-rust/slots/blue)/healthcheck.py"` (theo slot đích).
  - **Kèm theo:** thêm bước dừng green sau khi drain, dùng lệnh `ss` cụ thể.

- **D6 — Kiểm chứng rollback collaboration với document có attr mới** (staging, không cần code nếu không có lỗi)
  - **Câu hỏi:** collaboration bản cũ tải Yjs state có `ipaUK/ipaUS` thì báo lỗi `node.check`, hay bỏ attr rồi lưu đè (mất dữ liệu)?
  - **Việc:** ghi kết quả vào `deploy/README.md`. Nếu bị lưu đè thì quy tắc floor ở D2 cũng áp dụng cho `collaboration_sha`.

**Sửa lỗi review code Rust** (review 26/10/2026, làm theo thứ tự C1 → C3; mỗi mục một commit)

Kết quả review: không có lỗi bảo mật. Những phần đã làm tốt cần giữ nguyên:
- Access token và OAuth (PKCE, nonce, state).
- Bridge so secret constant-time.
- Storage media không bị path traversal.
- `grade`/`reveal` idempotent, khóa đúng thứ tự.

Các mục dưới đây là lỗi đã được đọc lại đúng dòng code. Đường dẫn tính từ `server/src/modules/`. Sau mỗi mục, chạy đủ ba lệnh ở A.7 của `docs/architecture.md`.

*C2 — Gộp logic bị chép (ưu tiên trung bình; đã bắt đầu lệch nhau)*

- **C2.6 `exercises::Question` lặp 5 field ở cả 7 variant**
  - **Vị trí:** `exercises/question.rs:8-119,290-298`.
  - **Sửa:** chuyển thành `struct Question { #[serde(flatten)] kind: Kind, … }`. JSON vào và ra phải giữ nguyên.
  - **Test:** dùng các test exercise hiện có cộng một test round-trip cho đủ 7 loại câu hỏi.
- **C2.7 Snapshot dự phòng của flashcard lệch giữa `log`, `reveal` và `grade`**
  - **Vị trí:** `vocabulary/flashcards/service/log.rs:146-147`, `service/review.rs:81,90,215-223`.
  - **Sửa:** tách một hàm `snapshot(&card, meanings)` dùng chung, theo đúng locale mặc định và cách gộp nghĩa.

*C3 — Dọn dẹp nhỏ (ưu tiên thấp, có thể gộp vài mục vào một commit cùng module)*

- **Comment bắt buộc theo A.3** cho các handler tự `map_err` lỗi extractor: `bulk/handlers.rs:13-14`, `ipa/handlers.rs:15-63`, `vocabulary/flashcards/handlers.rs`, `vocabulary/reviews/handlers.rs`, `vocabulary/library/handlers.rs:13`. Chỗ nào không cần thông báo riêng thì bỏ `map_err`, để `AppJson`/`AppQuery` tự trả lỗi.
- **JSONB đọc bằng index chuỗi:**
  - `cards.rs:64-66` đọc `run.settings["direction"]`, khi thiếu thì âm thầm dùng mặc định. Deserialize vào struct có kiểu.
  - `RunSummary.filters/settings`, `HistoryEntry.card` và `reviews::Card.source` trả struct thay vì `Value` (A.4).
- **Guard NaN:** `vocabulary/flashcards/scheduler.rs:45`. NaN đi qua `clamp`, ép kiểu thành 0, thẻ đến hạn ngay. Kiểm `is_finite` trước.
- **Gom hằng số** đang viết tay ở nhiều nơi:
  - Retention mặc định 0.9: `queue.rs:86`, `cards.rs:66`.
  - Giới hạn topic 60 và q 200: `library/service.rs:58`, `reviews/create.rs:26`, `flashcards/dto.rs:19`.
  - TTL 24 giờ của AI import: `ai_study/dto.rs:23`, `main.rs:73`.
  - `2_800_000` / `2_100_000` liên quan tới `3*1024*1024`: `collaboration.rs:83,92`, `internal.rs:21`.
- **Gom hàm bị chép:**
  - Mẫu hash `sha256(to_vec(...))`, có hai hàm cùng tên `key` khác nghĩa: `reviews/state.rs:11`, `queue.rs:15,28`, `flashcards/dto.rs:37`.
  - Validator tên: `places/dto.rs:24-48`, `courses/dto.rs:28-52`, `sessions/dto.rs:46`.
  - Làm sạch tên file: `media/service.rs:29`, `media/dto.rs:53`.
  - Lọc reference AI: `ai_study/service.rs:147-156,176-185`.
  - SQL đếm từ và bài tập: `courses/service.rs:196-200,237-238`.
  - Upsert trong settings: `settings/service.rs`.
- **Giá trị kiểu chuỗi** thay bằng enum:
  - Kind media `"file"`: `media/service.rs:143`, `recordings/service.rs:27,93`, `notes/document/mod.rs:12` dùng `media::dto::Kind`.
  - `"word"`/`"pair"`: `ipa/service.rs`.
  - Category settings: `settings/service.rs`.
- **Lặt vặt:**
  - `auth/handlers.rs:98`: đổi `auth.user.unwrap()` thành `.ok_or(AppError::Unauthenticated)?`.
  - `config.rs:115`: `optional()` bỏ qua chuỗi rỗng; giá trị mặc định `.storage/media` không chép sang `bin/media-cleanup.rs:6`.
  - `error.rs:110`: `Display` của `AppError` in đúng biến thể lỗi.
  - Bỏ `use` nằm giữa hàm: `places/service.rs:129,137`, `courses/service.rs:171,184`.
  - Gọi thừa `page_size()`: `sessions/service/mod.rs:99`.
  - Status 400 "đang dùng realtime" chuyển thành 409 theo A.4: `notes/collaboration.rs:30`, `notes/service/mod.rs:132,216`. Đây là thay đổi contract, phải ghi vào bảng.
  - `vocabulary/flashcards/service/statistics.rs:34-75`: bỏ transaction không cần thiết, INSERT/EXISTS viết bằng SeaQuery (`insert_many().on_conflict(do_nothing)`).

**Phát hành**
- Blue–green: thử rollback tương thích trên staging và smoke realtime hai cửa sổ qua listener 3010. Lượt production đầu đã qua CI và chuyển green → blue; bằng chứng ở `docs/features.md`.
- Smoke trên staging, rồi deploy production lần đầu có migration `m20261016_*` theo checklist trong `deploy/README.md` (kiểm tra `lower('É')`, cờ `breaking_migration`, backup).
- Thử đăng nhập Google thật end-to-end, và realtime hai người qua UI bằng tài khoản Google thật.

**Kỹ thuật**
- Xoay khóa JWT: giữ nhiều khóa trong JWKS để đổi khóa không làm đăng xuất người dùng.
- Realtime nhiều instance: hiện chỉ chạy một Hocuspocus, chưa có Redis coordination.

**Sản phẩm (chưa có API)**
- Lưu trữ/khôi phục nơi học và khóa học.
- Chuyển buổi học sang khóa khác.
- Sắp xếp thủ công.
- Tìm toàn văn trong nội dung ghi chú.
- Flashcard: import/export `.apkg`, giới hạn thẻ mỗi ngày, bury sibling, day cutoff/fuzz như Anki.
- Menu avatar ghi "Hồ sơ" nhưng màn chỉ hiển thị tài khoản: đổi nhãn thành "Tài khoản" hoặc làm chức năng sửa hồ sơ.
