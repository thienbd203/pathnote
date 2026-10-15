# Cài đặt cá nhân

Mở menu avatar → **Cài đặt cá nhân → Học tập** để chọn giờ bắt đầu và thời
lượng mặc định. Form tạo buổi học có nút chỉnh nhanh cùng nhóm cài đặt này.
Ví dụ lưu 18:00 / 90 phút, sau đó đổi thành 20:00 / 90 phút cho các buổi mới.
Không tự chuyển theo tháng, không sinh lịch và không sửa buổi học đã có.

`GET /api/v1/settings` trả các nhóm cài đặt của user hiện tại. Chưa lưu thì
nhóm `learning` trả 19:30 / 60 phút, giữ hành vi cũ.

`PUT /api/v1/settings/learning` nhận toàn bộ nhóm học tập:

```json
{"session_start_time":"18:00","session_duration_minutes":90}
```

API dùng Bearer JWT và không nhận user ID từ client. Giờ là HH:MM theo giờ địa
phương trên thiết bị, không phải timestamp UTC; buổi học thật vẫn gửi múi giờ
và chuyển sang UTC như trước. Khoảng giờ mặc định phải kết thúc cùng ngày.

Migration `m20261015_000001_create_user_settings` thêm bảng `user_settings` có
khóa chính `(user_id, category)`, `value` JSONB và `updated_at`. Mỗi nhóm có DTO
Rust riêng để validate; không có API cho phép ghi JSON tùy ý. Nhóm mới có thể
thêm độc lập, không ghi đè nhóm khác hoặc cần thêm cột cho mỗi tùy chọn.
Lưu một nhóm dùng upsert nguyên tử từ SeaORM.

Frontend đọc lại khi mở form tạo mới, hủy request khi đóng form, chờ trước khi
cho sửa giờ/thời lượng để response chậm không ghi đè dữ liệu vừa nhập. Form
sửa dùng dữ liệu buổi học. Cài đặt không lưu trong localStorage/cache chung
giữa tài khoản. Không tải được mặc định thì có thông báo và vẫn cho nhập tay.

Test API kiểm tra giá trị mặc định, lưu/đọc lại, đổi giờ, cách ly tài khoản,
giữ buổi học cũ và nhóm khác, Bearer và dữ liệu không hợp lệ.

Deploy Rust trước để áp migration và API mới, sau đó deploy web.
