# Việc còn mở

Cập nhật 18/10/2026. Đây là nơi duy nhất ghi việc chưa làm của toàn project.

Khi bắt đầu một mục, ghi kế hoạch ngắn ngay dưới mục đó; khi xong, cập nhật `docs/features.md` (hoặc `docs/architecture.md`) và xóa mục khỏi đây.

**Xác minh bản tối ưu R1–R8 trên CI**

R1–R8 đã triển khai, đo lại bằng Playwright và hạ ngân sách tương ứng. Hành vi hiện tại ở `docs/features.md`; ngân sách ở `pathnote-web/e2e/request-budgets.ts`.

- Sau khi push frontend và coordinator, xác minh browser gate của đúng cặp SHA mới xanh. Bộ test/gate gốc R0 đã qua trên GitHub ở run `39037695883`; không còn chờ xác minh R0.
- Không tăng ngân sách hoặc cập nhật ảnh chỉ để làm CI xanh. Khi bản tối ưu đã qua CI, xóa mục này.

**Phát hành**
- Chuyển VPS sang blue–green theo `deploy/README.md` mục 7; smoke một lượt deploy luân phiên, rollback tương thích và realtime qua listener nội bộ 3010. Test Linux/Nginx local đã qua, chưa xác minh cấu hình VPS.
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
