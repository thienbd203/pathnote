# Kế hoạch triển khai Pathnote Rust

Mỗi giai đoạn có một tài liệu riêng, ghi phạm vi, quyết định thiết kế, thứ tự
triển khai, mục tiêu học và điều kiện hoàn thành. Checklist chỉ đánh dấu khi
đã có code và kiểm tra tương ứng; tài liệu kế hoạch không phải tính năng đã làm.

| Giai đoạn | Nội dung | Trạng thái | Kế hoạch |
| --- | --- | --- | --- |
| 1 | Google login, session PostgreSQL, JWT access token | Đã có code và test API; chưa xác nhận Google login thật end-to-end | [README backend](../README.md) |
| 2 | Places, courses, sessions | Đã có CRUD, bulk API và tìm ngày; sinh lịch tự động đã bỏ | [Kế hoạch giai đoạn 2](02-learning-hierarchy.md) |
| 3 | Ghi chú JSONB và chia sẻ | Đã có API và editor autosave; bảy dạng bài tập đã nối | [Ghi chú và chia sẻ](03-notes-and-sharing.md) |
| 3b | Ảnh và tệp riêng tư | Upload/read, kiểm tra media reference; dọn orphan mỗi giờ với ngưỡng 24 giờ | [Private media](03b-private-media.md) |
| 3c | Realtime Hocuspocus → Rust | Test hai client, persistence và thu hồi quyền đã qua; cần thử UI với Google thật | [Realtime bridge](03c-realtime.md) |
| 4 | Kho từ vựng từ ghi chú | Đồng bộ cùng transaction, lọc và frontend | [Kho từ vựng](04-vocabulary-library.md) |
| 5 | IPA/dịch và flashcard | API, UI và tiến độ riêng; service local thật đã test | [Flashcard](05-vocabulary-reviews.md) |

Rà soát mới nhất và thứ tự hoàn thiện:
[Kế hoạch giai đoạn 6–9](06-completion-roadmap.md).
Đã nối [bài tập từng node](06a-inline-exercises.md).
Đã nối [Học cùng AI](07-ai-study.md) với prepare/retry, autosave/Yjs và cleanup.
Đã nối [IPA, block phát âm và bulk UI](08-ipa-and-bulk-ui.md).
Đã nối [ôn flashcard FSRS](09-anki-fsrs.md) với hai chiều có lịch riêng.
Chuẩn kiến trúc và kế hoạch chuẩn hóa BE/FE: [giai đoạn 10](10-architecture.md).
Bước tiếp theo: smoke qua browser, staging và chuẩn bị CI/CD Rust.
Các tài liệu 02a/02b/02c giữ nội dung bước học tại thời điểm triển khai;
trạng thái hiện tại của hierarchy nằm trong [giai đoạn 2](02-learning-hierarchy.md).

Nguyên tắc chung:

- Tìm và dùng chức năng có sẵn của package trước khi tự viết hạ tầng.
- Giữ workspace `server` và `migration`, chưa tách thêm crate.
- Tập trung test qua HTTP API, tách theo chức năng, helper dựng app dùng chung.
- Laravel là nguồn tham khảo hành vi, không sao chép máy móc kiến trúc framework.
- Dùng JWT cho API nghiệp vụ; phiên cookie chỉ phục vụ luồng đăng nhập,
  refresh và logout.
- Không chuyển frontend/collaboration hoặc dữ liệu production trong một bước
  backend nếu chưa có yêu cầu riêng.
