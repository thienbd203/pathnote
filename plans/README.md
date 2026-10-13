# Kế hoạch triển khai Pathnote Rust

Mỗi giai đoạn có một tài liệu riêng, ghi phạm vi, quyết định thiết kế, thứ tự
triển khai, mục tiêu học và điều kiện hoàn thành. Checklist chỉ đánh dấu khi
đã có code và kiểm tra tương ứng; tài liệu kế hoạch không phải tính năng đã làm.

| Giai đoạn | Nội dung | Trạng thái | Kế hoạch |
| --- | --- | --- | --- |
| 1 | Google login, session PostgreSQL, JWT access token | Đã có code và test API; chưa xác nhận Google login thật end-to-end | [README backend](../README.md) |
| 2 | Places, courses, sessions và sinh lịch ban đầu | Đã có CRUD ba cấp; sinh lịch và bulk-delete chưa triển khai | [Kế hoạch giai đoạn 2](02-learning-hierarchy.md) |
| 3 | Ghi chú JSONB và chia sẻ | Đã có API và editor autosave; realtime/bài tập chưa nối | [Ghi chú và chia sẻ](03-notes-and-sharing.md) |
| 3b | Ảnh và tệp riêng tư | Upload/read và kiểm tra media reference; chưa có orphan cleanup job | [Private media](03b-private-media.md) |

Bước thực hành đầu tiên: [Học Rust qua Places](02a-places.md). Triển khai từng
module để đọc hiểu code trước khi mở rộng; không tạo cả hierarchy trong một bước.

Bước tiếp theo: [Courses, relation và transaction](02b-courses.md).

Bước thứ ba: [Sessions, thời điểm UTC và relation tùy chọn](02c-sessions.md).

Nguyên tắc chung:

- Tìm và dùng chức năng có sẵn của package trước khi tự viết hạ tầng.
- Giữ workspace `server` và `migration`, chưa tách thêm crate.
- Tập trung test qua HTTP API, tách theo chức năng, helper dựng app dùng chung.
- Laravel là nguồn tham khảo hành vi, không sao chép máy móc kiến trúc framework.
- Dùng JWT cho API nghiệp vụ; phiên cookie chỉ phục vụ luồng đăng nhập,
  refresh và logout.
- Không chuyển frontend/collaboration hoặc dữ liệu production trong một bước
  backend nếu chưa có yêu cầu riêng.
