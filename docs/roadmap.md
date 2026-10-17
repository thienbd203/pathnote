# Việc còn mở

Cập nhật 17/10/2026. Đây là nơi duy nhất ghi việc chưa làm của toàn project.

Khi bắt đầu một mục, ghi kế hoạch ngắn ngay dưới mục đó; khi xong, cập nhật `docs/features.md` (hoặc `docs/architecture.md`) và xóa mục khỏi đây.

**Hiệu năng request frontend (làm tiếp theo)**

Báo cáo đo tay ngày 17/10/2026 (frontend `8aaae7a`) cho thấy nhiều màn gọi API thừa, nhất là trên mobile. Phép đo đó không chạy lại được. Làm theo thứ tự: dựng bộ đo trước (R0), sau đó mỗi mục sửa (R1–R8) phải hạ ngân sách tương ứng trong cùng commit.

- **R0 — Xác minh CI cho bộ kiểm thử tự động vừa triển khai**:
  - Bộ đo request, ảnh chuẩn desktop/mobile, schema PostgreSQL riêng và gate đúng cặp SHA đã có; hành vi ghi trong `docs/features.md`, cách chạy trong README web.
  - Chạy lần đầu trên GitHub với SHA frontend mới trong manifest; chỉ đánh dấu xong khi job request/visual và browser contract xanh. Không tự nâng ngân sách hoặc cập nhật ảnh để làm CI xanh.
  - Sau khi xác minh, xóa mục này. R1–R8 giữ nguyên thứ tự bên dưới.

- **Sửa theo báo cáo**, mỗi mục một commit và hạ ngân sách tương ứng:
  - Kế hoạch R3: trả run summary trong next/grade, gom aggregate summary thành một truy vấn; dùng response cập nhật UI/cache, không refetch nền từng thẻ. Giữ nguyên ticket/reveal/lịch ôn.
  - **R3** Vòng flashcard:
    - Không GET run hai lần.
    - `grade`/`next` cập nhật summary và overview từ response.
    - Invalidate các màn khác mà không refetch ngay query không cần.
    - Giữ nguyên ticket, quyền, FSRS và bước reveal: `VocabularyFlashcardsView.vue:241,260`, `lib/api/flashcards.ts:276`.
  - **R4** Thư viện: thống kê khóa trả kèm list, hoặc dùng endpoint bulk (chỉ số buổi và buổi gần nhất). Danh sách buổi chỉ tải khi vào khóa: `LibraryPlaceGroup.vue:26,32`, `lib/api/courses.ts:259`. Đây là thay đổi contract, phải ghi vào `docs/architecture.md`.
  - **R5** Mở gần đây:
    - Lấy dữ liệu từ timeline đã tải.
    - Bulk đọc các ID còn thiếu, giữ thứ tự.
    - Loại ID đã mất quyền hoặc đã xóa.
    - Vị trí: `LearnView.vue:38`.
  - **R6** Chip ẩn/ghim lấy tên từ catalog hoặc place list. Chỉ bulk tải ID còn thiếu, không tải từ lúc khởi động: `useNavigationPreferences.ts:29`.
  - **R7** Tab Lịch sử flashcard không tải overview và recent của tab Ôn tập: `FlashcardsView.vue:27`.
  - **R8** IPA chỉ tải bộ dữ liệu đang xem. Kiểm tra trùng khi import thì đối chiếu riêng các mục được nhập: `IpaView.vue:87`, `lib/api/ipa.ts:12`.

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
