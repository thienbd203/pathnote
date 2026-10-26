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

**Animation lật thẻ flashcard — kiểm tra Safari thật**

Bản WAAPI và bố cục giữ chiều cao đã triển khai; thông số ở `pathnote-web/docs/ui.md`, hành vi ở `docs/features.md`. Còn thử tay Safari macOS/iOS thật; kiểm thử WebKit/iPhone giả lập không thay cho kiểm tra thiết bị thật. Không chạy riêng kịch bản giảm chuyển động trong lượt kiểm tra 26/10 theo yêu cầu.

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
