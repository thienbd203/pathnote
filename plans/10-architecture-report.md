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
- Frontend: 190 bài kiểm thử qua, build và Prettier check qua. Kiểm thử bổ sung xác nhận ba consumer overview Flashcard dùng chung một request, dùng lại cache còn mới và chỉ phát sinh một request khi invalidate.
- Trình duyệt localhost: kiểm tra desktop và mobile 390 × 844 cho Flashcard, lịch sử ôn, onboarding và drawer chuyển buổi. Đã mở note được chia sẻ, xác nhận nội dung và trạng thái “Đã đồng bộ”.
- Tài khoản localhost không có buổi học riêng; chưa xác nhận trực quan card khóa học có dữ liệu riêng. Contract thống kê được kiểm thử qua API. Không tạo dữ liệu hoặc gửi lượt ôn thử trong tài khoản của người dùng.
- Có lỗi kết nối tạm thời khi tiến trình phát triển đang build lại backend; trang hoạt động lại sau khi server sẵn sàng.

## Lỗi chấm câu dịch phát hiện thêm

Đáp án mẫu có dấu chấm cuối câu nhưng câu trả lời không có dấu chấm bị báo sai. Đã sửa riêng bài dịch để dấu chấm cuối là tùy chọn. Vẫn kiểm tra nội dung, dấu sở hữu và cấu hình phân biệt hoa/thường; bài sửa lỗi câu và điền từ giữ quy tắc dấu câu hiện tại. Bổ sung kiểm thử qua HTTP cho câu của Anna, các đáp án sai và các quy tắc cần giữ. Không sửa lại kết quả đã lưu trước đó.

Các file hướng dẫn và kế hoạch có sẵn trong working tree của người dùng không được đưa vào commit triển khai.

## Đối chiếu từng mục kế hoạch

Tất cả các mục triển khai dưới đây đã hoàn thành. Hash ở bảng là commit triển khai chính; các commit nghiệm thu và sửa race sau đó được nêu ở cuối.

| Backend | Commit |
| --- | --- |
| B.1.1 Extractor, lỗi JSON | `c9e6827` |
| B.1.2 Phân trang, escape LIKE | `b32a44a` |
| B.1.3 ApiResponse.message | `bd753d4` |
| B.1.4 Middleware, timeout, pool | `e8df935` |
| B.1.5 write_tx | `f9ec0be` |
| B.2.1 Places, courses, sessions | `d172207` |
| B.2.2 Course library | `db7ccec` |
| B.2.3 Session search | `0a9c6ef` |
| B.2.4 Notes, collaboration | `677d0df` |
| B.2.5 Media DTO | `3cf9858` |
| B.2.6 Enrichment | `8a6dd1a` |
| B.2.7 Exercises, AI study | `bbe3d85` |
| B.2.8 IPA, settings | `1c11ba7` |
| B.2.9 Vocabulary library | `5a1900a` |
| B.2.10 Reviews | `b910fca` |
| B.2.11 Flashcards | `c7e7192` |
| B.3.1 Index | `daed287` |
| B.3.2 Note metadata | `3236adf` |
| B.3.3 Save note | `36da575` |
| B.3.4 Flashcard next | `80986a1` |
| B.3.5 Vocabulary queries | `fb6cf21` |
| B.3.6 Neighbors | `075b65d` |
| B.3.7 Batch questions | `d98a49b` |
| B.3.8 Review parent locks | `f6e9a8c` |
| B.3.9 Media stream | `8dc177d` |
| B.4.1 Course summary | `fdb2937` |
| B.4.2 Lightweight overview | `80986a1`, `90f14b5` |

| Frontend | Commit |
| --- | --- |
| Prettier | `9e5b87c` |
| B.1.1 Vue Query, API client | `713d785` |
| B.1.2 toApiError | `c5c8e07` |
| B.1.3 Single timeline / polling mount | `66e67b2` |
| B.2 Settings | `028ff13` |
| B.3 Places | `652d8d6` |
| B.3 Courses | `959aec3`, `4ef9df8` |
| B.3 Sessions | `e554232` |
| B.3 Workspace | `e2275ee` |
| B.3 Navigation preferences | `250da55` |
| B.3 Store cleanup | `d1e6ac6` |
| B.4.1 Vocabulary | `5cd9cfe`, `6de72ea` |
| B.4.2 Flashcards | `ed5d659` |
| B.4.3 Reviews | `9c2d4c5` |
| B.4.4 IPA | `127bf34`, `ea349f9` |
| B.4.5 Enrichment | `9e705fd` |
| B.4.6 Exercises / AI | `3c0e853` |
| B.5 Notes / sharing | `55278c1` |
| B.5 Shared notes | `b38e88b` |
| B.5 Media | `6c18458` |
| B.5 Auth | `5efbe93` |

Nghiệm thu bổ sung: backend `07dd604`, `b951f76`; frontend `a01b726`, `42acbff`, `8712407`. Việc đo request trùng trên tab Network của trình duyệt vẫn chưa được xác nhận trực tiếp; kiểm thử tự động đã xác nhận request dùng chung cho neighbors và overview Flashcard. Những phần ghi “Không làm trong đợt này” ở hai kế hoạch không được triển khai.
