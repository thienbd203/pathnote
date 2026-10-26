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

*C3 — Dọn dẹp nhỏ (ưu tiên thấp, có thể gộp vài mục vào một commit cùng module)*

- **C3.4 Gom hằng số** đang viết tay ở nhiều nơi:
  - Retention mặc định 0.9: `queue.rs:86`, `cards.rs:66`.
  - Giới hạn topic 60 và q 200: `library/service.rs:58`, `reviews/create.rs:26`, `flashcards/dto.rs:19`.
  - TTL 24 giờ của AI import: `ai_study/dto.rs:23`, `main.rs:73`.
  - `2_800_000` / `2_100_000` liên quan tới `3*1024*1024`: `collaboration.rs:83,92`, `internal.rs:21`.
- **C3.5 Gom hàm bị chép:**
  - Mẫu hash `sha256(to_vec(...))`, có hai hàm cùng tên `key` khác nghĩa: `reviews/state.rs:11`, `queue.rs:15,28`, `flashcards/dto.rs:37`.
  - Validator tên: `places/dto.rs:24-48`, `courses/dto.rs:28-52`, `sessions/dto.rs:46`.
  - Làm sạch tên file: `media/service.rs:29`, `media/dto.rs:53`.
  - Lọc reference AI: `ai_study/service.rs:147-156,176-185`.
  - SQL đếm từ và bài tập: `courses/service.rs:196-200,237-238`.
  - Upsert trong settings: `settings/service.rs`.
- **C3.6 Giá trị kiểu chuỗi** thay bằng enum:
  - Kind media `"file"`: `media/service.rs:143`, `recordings/service.rs:27,93`, `notes/document/mod.rs:12` dùng `media::dto::Kind`.
  - `"word"`/`"pair"`: `ipa/service.rs`.
  - Category settings: `settings/service.rs`.
- **C3.7 Lặt vặt:**
  - `auth/handlers.rs:98`: đổi `auth.user.unwrap()` thành `.ok_or(AppError::Unauthenticated)?`.
  - `config.rs:115`: `optional()` bỏ qua chuỗi rỗng; giá trị mặc định `.storage/media` không chép sang `bin/media-cleanup.rs:6`.
  - `error.rs:110`: `Display` của `AppError` in đúng biến thể lỗi.
  - Bỏ `use` nằm giữa hàm: `places/service.rs:129,137`, `courses/service.rs:171,184`.
  - Gọi thừa `page_size()`: `sessions/service/mod.rs:99`.
  - Status 400 "đang dùng realtime" chuyển thành 409 theo A.4: `notes/collaboration.rs:30`, `notes/service/mod.rs:132,216`. Đây là thay đổi contract, phải ghi vào bảng.
  - `vocabulary/flashcards/service/statistics.rs:34-75`: bỏ transaction không cần thiết, INSERT/EXISTS viết bằng SeaQuery (`insert_many().on_conflict(do_nothing)`).

**Animation lật thẻ flashcard** (repo `pathnote-web`, chốt 27/10/2026)

Mục tiêu: màn ôn `/flashcards/study` (`src/views/VocabularyFlashcardsView.vue`) có chuyển động như bản demo đã duyệt (Artifact "Lật thẻ Pathnote"). Logic ôn giữ nguyên: ticket, `reveal`/`grade`, phím tắt, FSRS, request.

1. **Thư viện:** dùng Motion for Vue (`motion-v`, bản 2.x mới nhất), không tự viết bằng Web Animations API. Dùng các API: `motion.*`, `AnimatePresence`, transition `type: 'spring'`, `MotionConfig reducedMotion="user"`, `useMotionValue`.
2. **Cấu trúc thẻ:**
   - Một khối `flipper` chứa hai mặt xếp chồng bằng CSS grid (`grid-area: 1/1`), nên chiều cao thẻ bằng mặt cao hơn.
   - Cả hai mặt có `backface-visibility: hidden`; mặt sau xoay sẵn `rotateY(180deg)`.
   - Mặt nào đang ẩn thì đặt `aria-hidden="true"`.
   - Nội dung mặt sau chỉ render sau khi `reveal` trả về. **Không gọi API sớm, không lộ đáp án trước.**
3. **Chờ API khi bấm lật:** thẻ thu nhỏ `scale .985` và lắc `rotateY` ±6° lặp lại cho tới khi có response. Nút chuyển sang "Đang lật…" như hiện tại.
4. **Lật:**
   - `rotateY` từ 0 lên 180, spring `stiffness 170, damping 18`.
   - Cùng lúc thẻ nhấc lên (`translateZ 40px`, `scale 1.03`) và bóng đổ sâu hơn ở khoảng 45% thời gian, rồi hạ xuống.
   - Một vệt sáng quét ngang mặt sau (CSS keyframe, 0,75 giây).
   - Các dòng đáp án hiện lần lượt, mỗi dòng trễ 70 ms, `opacity` 0→1 kèm `y` 10→0.
   - Bốn nút chấm hiện lần lượt, mỗi nút trễ 40 ms.
   - Chiều cao thẻ đổi mượt bằng `layout`.
5. **Chấm điểm:**
   - Viền thẻ nháy màu của mức chấm trong 420 ms.
   - Thẻ bay ra theo mức chấm, dùng `AnimatePresence` với key là ID ticket và prop `custom` là mức chấm, thời gian 420 ms, ease-in:
     - Quên: giật nhẹ rồi rơi xuống trái (`x -140, y 90, rotate -14°`).
     - Khó: trượt trái (`x -160, rotate -7°`).
     - Nhớ: trượt phải (`x 160, rotate 7°`).
     - Dễ: bay lên phải (`x 150, y -120, rotate 12°, scale .95`).
   - Thẻ mới trồi lên từ chồng bài: từ `y 11, scale .955, opacity .7` về vị trí chuẩn bằng spring `stiffness 260, damping 22`.
6. **Chồng bài:** hai lớp thẻ mờ phía sau (`translateY 11px scale .955 opacity .75` và `translateY 22px scale .91 opacity .45`), giật nhẹ khi thẻ trên cùng rời đi. Khi lượt ôn còn ít thẻ hơn thì ẩn bớt lớp nếu dữ liệu cho biết số thẻ còn lại; nếu không có dữ liệu đó thì luôn hiện cả hai.
7. **Nghiêng theo chuột:** chỉ khi `(hover: hover) and (pointer: fine)`, tối đa 6°, trả về 0 khi chuột rời thẻ. Tắt khi đang chờ API hoặc đang có animation.
8. **Chạm để lật:** cả thẻ là vùng bấm khi chưa lật, có `role="button"` và `aria-label`. Nút "Hiện đáp án" và phím Space/Enter vẫn giữ. Phím 1–4 không chạy khi đang có animation, hoặc giữ nguyên guard `busy`/`graded` hiện có.
9. **Giảm chuyển động:** đi theo hệ điều hành qua `MotionConfig reducedMotion="user"`. Khi bật thì chỉ mờ chéo 150 ms, không xoay, không bay, không nghiêng. Kiểm tra bằng `page.emulateMedia({ reducedMotion: 'reduce' })`.
   - **Lỗi đã gặp trong demo:** sau khi làm mờ mặt trước, thẻ bị trắng hẳn vì giá trị opacity 0 của animation bị ghi lại vào style. Thêm test e2e: ở chế độ giảm chuyển động, sau chu trình lật → chấm, mặt trước của thẻ kế tiếp vẫn hiện.
10. **Không đổi:** API, số request (ngân sách `e2e/request-budgets.ts` không được tăng), luồng lỗi và thử lại, phát âm, lịch sử.
11. **Kiểm tra:**
    - `npm test`, `npm run build`, `npm run format:check`, `npm run e2e`.
    - Ảnh visual của màn ôn chụp với `reducedMotion: 'reduce'` để ổn định. Nếu ảnh thay đổi thì cập nhật và nêu lý do.
    - Thử tay trên Chrome và Safari (desktop và iOS), vì `backface-visibility` và `preserve-3d` hay lỗi trên Safari.
12. **Tài liệu:** thêm mục "Chuyển động" vào `docs/ui.md` gồm thông số spring, thời gian, quy tắc giảm chuyển động và quy tắc chỉ lật sau khi API trả về.

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
