# Việc còn mở

Cập nhật 17/10/2026. Đây là nơi duy nhất ghi việc chưa làm của toàn project.

Khi bắt đầu một mục, ghi kế hoạch ngắn ngay dưới mục đó; khi xong, cập nhật `docs/features.md` (hoặc `docs/architecture.md`) và xóa mục khỏi đây.

**Xác minh bộ kiểm thử hiệu năng trên CI**

R1–R8 đã triển khai, đo lại bằng Playwright và hạ ngân sách tương ứng. Hành vi hiện tại ở `docs/features.md`; ngân sách ở `pathnote-web/e2e/request-budgets.ts`. Còn bước xác minh GitHub CI dưới đây.

- **R0 — Xác minh CI cho bộ kiểm thử tự động vừa triển khai**:
  - Bộ đo request, ảnh chuẩn desktop/mobile, schema PostgreSQL riêng và gate đúng cặp SHA đã có; hành vi ghi trong `docs/features.md`, cách chạy trong README web.
  - Chạy lần đầu trên GitHub với SHA frontend mới trong manifest; chỉ đánh dấu xong khi job request/visual và browser contract xanh. Không tự nâng ngân sách hoặc cập nhật ảnh để làm CI xanh.
  - Sau khi xác minh, xóa mục này.


**Phát hành**
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
