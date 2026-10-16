# Báo cáo chuẩn hóa kiến trúc — 16/10/2026

Đã thực hiện B.1–B.4 của kế hoạch backend và B.1–B.5 của kế hoạch frontend. Các bước được commit riêng trong hai repository. Chưa push hoặc triển khai production trong đợt này.

## Kết quả

- Backend tách route, handler, service và DTO; dùng extractor/lỗi JSON chung, transaction và thứ tự khóa chung. Response có kiểu rõ ràng; JSON tự do của tài liệu note được giữ riêng.
- Tối ưu đọc metadata note, lưu note, truy vấn Flashcard, kho từ, buổi liền trước/sau, ghi hàng loạt và đọc media theo luồng. Bổ sung index và endpoint thống kê khóa học.
- Frontend tập trung endpoint vào các module API, dùng Vue Query cho dữ liệu máy chủ và Pinia cho trạng thái phía người dùng. Chuẩn hóa cache, debounce, polling và invalidation; hủy lượt đọc cũ trước khi công bố kết quả ghi để tránh ghi đè dữ liệu mới.
- Giữ luồng autosave, xử lý xung đột phiên bản, collaboration và tiến độ upload. Thêm Prettier, script format/check và định dạng source frontend.
- Các thay đổi contract được ghi ở B.6 của kế hoạch backend và cập nhật phía frontend: phân trang, kết quả import IPA, thống kê khóa học và số lượng Flashcard.

## Hiệu năng đã đo và đối chiếu

| Luồng | Trước | Sau | Cách xác nhận |
| --- | --- | --- | --- |
| Flashcard next, một chiều | 13 SQL | 7 SQL | SQL log của bài kiểm thử profile |
| Flashcard next, hai chiều | 19 SQL | 8 SQL | SQL log của bài kiểm thử profile |
| Flashcard next, lượt ôn đã lưu, hai chiều | 19 SQL | 8 SQL | SQL log của bài kiểm thử profile |
| Kho từ | 7 truy vấn dữ liệu | 2 truy vấn dữ liệu | Đối chiếu service; không tính kiểm tra quyền của bộ lọc |
| Card thống kê khóa học | Hai request tìm buổi phụ | Dùng kết quả library-stats | Đối chiếu API frontend và kiểm thử contract backend |

Các số SQL Flashcard là trên bộ dữ liệu kiểm thử một buổi học, không phải số đo production. Không đo số request bằng Network của trình duyệt trong đợt này.

## Kiểm tra

- Backend: toàn bộ workspace test qua; nhóm API có 96 bài qua, 2 bài bị đánh dấu bỏ qua. Các nhóm kiểm thử phân trang, extractor và response cũng qua. Clippy không có cảnh báo, kiểm tra định dạng qua.
- Frontend: 189 bài kiểm thử qua, build và Prettier check qua.
- Trình duyệt localhost: kiểm tra desktop và mobile 390 × 844 cho Flashcard, lịch sử ôn, onboarding và drawer chuyển buổi. Đã mở note được chia sẻ, xác nhận nội dung và trạng thái “Đã đồng bộ”.
- Tài khoản localhost không có buổi học riêng; chưa xác nhận trực quan card khóa học có dữ liệu riêng. Contract thống kê được kiểm thử qua API. Không tạo dữ liệu hoặc gửi lượt ôn thử trong tài khoản của người dùng.
- Có lỗi kết nối tạm thời khi tiến trình phát triển đang build lại backend; trang hoạt động lại sau khi server sẵn sàng.

## Lỗi chấm câu dịch phát hiện thêm

Đáp án mẫu có dấu chấm cuối câu nhưng câu trả lời không có dấu chấm bị báo sai. Đã sửa riêng bài dịch để dấu chấm cuối là tùy chọn. Vẫn kiểm tra nội dung, dấu sở hữu và cấu hình phân biệt hoa/thường; bài sửa lỗi câu và điền từ giữ quy tắc dấu câu hiện tại. Bổ sung kiểm thử qua HTTP cho câu của Anna, các đáp án sai và các quy tắc cần giữ. Không sửa lại kết quả đã lưu trước đó.

Các file hướng dẫn và kế hoạch có sẵn trong working tree của người dùng không được đưa vào commit triển khai.
