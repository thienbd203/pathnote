# Xóa hàng loạt và tìm buổi học theo ngày

## Xóa nguyên nhóm

`modules/bulk.rs` có enum Kind cho ba tài nguyên cụ thể của Pathnote. DTO nhận
Vec<i64>; kiểm tra số lượng, ID dương và không lặp. Không dùng generic repository.

1. Dùng ownership scope đã có để đọc tất cả ID trong một query, lấy tập place IDs.
2. Mở transaction, khóa các place theo BTreeSet (thứ tự tăng dần).
3. Đọc lại tập ID sau khi lấy lock để kiểm tra xóa cha hoặc thay đổi đồng thời.
4. Thiếu bất kỳ ID nào thì trả 404 trước khi cập nhật.
5. UpdateMany đánh dấu con và cha trong cùng transaction rồi commit.

Mọi thao tác tạo/sửa/xóa con, ghi note và chia sẻ trước đó cũng khóa place trước,
nên bulk không bỏ qua phối hợp đã có. Không xóa bytes của ảnh hoặc nội dung note.
Xóa mềm chỉ làm chúng không còn truy cập được qua API.

## Tìm ngày theo timezone

`modules/sessions/search.rs` dùng Chrono và chrono-tz hiện có, không tự tính UTC offset.
`13/10/2026` là ngày lịch địa phương; lấy midnight của ngày đó và midnight ngày
kế tiếp rồi chuyển từng mốc sang UTC. Không cộng 24 giờ vào mốc đầu vì ngày DST
có thể chỉ 23 hoặc 25 giờ.

Query SQL là:

```text
owner + active parents + optional course
AND (title LIKE literal pattern OR starts_at >= start AND starts_at < end)
```

Nhóm OR không bao ngoài owner, tránh lộ dữ liệu của tài khoản khác khi match ngày.
Escape %, _ và ! cho tìm tên; dùng SeaORM bind parameters.
Ngày d/m không có năm lấy năm hiện tại trong timezone được gửi. Ngày nhập sai
không tạo range nhưng vẫn được dùng làm text search. Midnight không chuyển được
duy nhất trả lỗi 422, không panic hoặc tự chọn một offset.

Response dùng SessionResource và paginator đã có, lấy place/course bằng JOIN.
Không lấy dữ liệu trên trang đầu rồi lọc ở frontend.

## Frontend

Dùng lại SessionNavigation và SessionSearchDialog: chọn nhiều buổi học, xác nhận
xóa, tìm theo tên/ngày và phạm vi khóa hoặc mọi buổi. Khi chọn buổi đang mở để
xóa, autosave phải hoàn tất trước. DELETE gửi một request chứa cả ids, không gửi
nhiều DELETE rời tạo tình trạng xóa một phần.

UI chọn nhiều nơi/khóa đã được bổ sung ở [bước 8](08-ipa-and-bulk-ui.md).

## Test

Test HTTP kiểm tra batch lẫn owner/missing/deleted ID không xóa một phần,
cascade giữ timestamp cũ, request đồng thời đảo thứ tự ID không deadlock,
ngày địa phương khác UTC, DST 23 giờ, ngày bị bỏ qua, literal wildcard và phân trang.
