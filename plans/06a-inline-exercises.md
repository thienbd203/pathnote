# Bài tập từng node — bảy dạng câu hỏi

## Trạng thái

Đã có API Rust, migration SeaQuery, reference validation trong REST/Yjs, và
frontend mở mục **Bài tập** ở slash menu. Mỗi câu một block, hiện đồng thời trong
bài học. Đã mở choice, fill, classify, matching, ordering, translation, correction.
Học cùng AI đã nối ở [bước 7](07-ai-study.md). Đã áp migration vào database local, chưa deploy production.

## Thử trên frontend local

1. Mở một buổi học, gõ `/` và chọn **Bài tập**.
2. Bấm **Sao chép JSON mẫu**, dán lại hoặc nhờ ChatGPT tạo JSON theo mẫu.
3. Bấm **Xem trước**, rồi **Lưu bài tập**. Bảy câu mẫu được chèn thành bảy node.
4. Trả lời ngay trong block; sai lần đầu còn một lần thử, câu kết thúc khi đúng
   hoặc sai lần hai. Đáp án và giải thích chỉ xuất hiện sau khi kết thúc.
5. Menu của từng câu có **Làm lại** và **Sửa đề** nếu có quyền. F5 giữ lượt đang làm.

Mỗi người có lượt riêng. Yjs chỉ đồng bộ ID đề, không đồng bộ đáp án người học.
Người tạo đề và owner đang còn quyền note được đọc/sửa definition đầy đủ; người
học khác chỉ nhận projection không có đáp án. Đây là bài luyện tập, không phải thi.

## Đọc code để học Rust

- `migration/src/m20261014_020001_create_exercises.rs`: hai bảng exercises và
  exercise_attempts, foreign keys và index tìm lượt mới nhất.
- `modules/exercises/question.rs`: enum Question tagged Serde cho bảy dạng;
  validate luật từng dạng và projection public không có đáp án.
- `dto.rs`: request tạo đề/import/lượt làm/trả lời; definition validation.
- `service.rs`: resource scope, quyền quản lý, revision và state hai lần thử.
  Đọc không khóa place; mutation lấy parent lock theo cùng thứ tự với note/revoke.
  Validate đề ngoài transaction; check quyền/revision/reference trong transaction.
- `mod.rs`: routes, CurrentUser extractor, JsonRejection → 422; body tối đa 256 KiB.
- `notes/document/node.rs`: Node::Exercise chỉ chứa attrs.exerciseId.
  `document::Reference` tách media/exercise, không nhét đáp án vào document.
- `notes/service.rs`: reference phải thuộc cùng buổi học khi lưu REST hoặc Yjs.
- `tests/api/exercises.rs` và `exercise_types.rs`: test qua HTTP theo luồng quyền,
  lượt làm và từng dạng; không test riêng service/helper.

Không thêm thư viện quiz/queue hoặc thay framework. Serde, SeaORM/SeaQuery, Axum
và Hocuspocus thực hiện phần hạ tầng; luật quyền/chấm luyện tập là logic sản phẩm.

## API

Prefix `/api/v1/sessions/{session}/exercises`, Bearer JWT:

| Method/path | Nội dung |
| --- | --- |
| POST `/` | Tạo một câu, trả id |
| POST `/import` | Tạo 1–100 câu trong cùng transaction, trả ids theo thứ tự |
| GET `/{id}` | Đề public và lượt hiện tại của người gọi |
| GET `/{id}/definition` | Đề/đáp án cho người được quản lý |
| PUT `/{id}` | Sửa một câu với revision hiện tại |
| POST `/{id}/attempts` | Tiếp tục lượt, hoặc restart=true để làm lại |
| POST `/{id}/attempts/{attempt}/answers` | position=0, answer, try=0 hoặc 1 |

Payload tạo/sửa/import: title, questions; revision chỉ cho sửa; titles tùy chọn
cho import và phải có đúng số câu. Mỗi resource chứa đúng một question. choice
có 2–6 options khác nhau và một answer trong options. fill có options rỗng và
1–10 đáp án mẫu. Field lạ và dạng chưa hỗ trợ bị từ chối. Prompt <=2000 ký tự,
option <=200, answer <=1000, explanation <=6000, title <=200.

classify dùng lựa chọn như choice. matching cần 2–8 items khác nhau ở cột trái;
options/answers có cùng số mục, khác nhau và thuộc cùng tập; answers theo thứ tự
items. Câu trả lời là chuỗi JSON của mảng phương án; tối đa 4000 ký tự, kiểm tra
đủ cặp và không dùng lại phương án. Chấm theo thứ tự cặp, không theo thứ tự options.
ordering có 2–20 từ; câu phải dùng đúng multiset các từ, cho phép từ trùng.
Đáp án mẫu dùng đủ từ và đúng dấu câu; input thiếu/thừa/tự thêm từ trả 422,
thứ tự sai hợp lệ tính một lần thử.
translation/correction giống fill: options rỗng, nhiều đáp án mẫu được chấp nhận.
Không dùng AI chấm ngữ nghĩa; caseSensitive áp dụng cho câu trả lời văn bản,
trim và gom whitespace khi so sánh. choice/classify/matching chọn phương án chính
xác theo chuỗi trong đề; không đồng nhất hai lựa chọn khác nhau chỉ vì khác hoa/thường.
Input không hợp lệ trả 422 và không tiêu hao lần thử.


Retry cùng answer/try cho cùng lượt trả lại kết quả, không tăng số lần thử.
Sửa đề tăng revision; trả lời lượt cũ hoặc lượt đã thay bởi restart trả 409.
Thiếu quyền/cha xóa mềm/câu thuộc bài khác trả 404; JWT không có trả 401.
Reference không thuộc bài trong document trả 422.

Import resources và chèn node/autosave là hai operation hiện tại. Nếu đóng trang
trước khi lưu note có thể còn resource không tham chiếu; không xóa ngay để giữ
undo. Import API chưa có idempotency key. Bước Học cùng AI sẽ xử lý phối hợp import
với lưu tài liệu; không tuyên bố hai operation đã atomic.

## Kiểm tra

- 69 API test thông thường qua. Test hai client Hocuspocus thật đã chạy riêng qua
  trong bước 6a; chưa chạy lại vì phần bridge không thay đổi ở bước 6b.
- Realtime test tạo node, đồng bộ hai client, lưu/mở lại không nhân đôi; collaborator
  hoàn thành mà owner chưa có lượt. Gỡ cộng tác viên ngắt kết nối như trước.
- 158 frontend test qua, typecheck/build qua; Clippy/fmt sạch.
- Chưa test thao tác UI với Google thật trong đợt này. Bước smoke browser vẫn
  nằm trong kế hoạch, không coi API/Node client test là browser end-to-end.

```sh
cargo run -p migration -- up
cargo test --workspace
NODE_BIN=/opt/homebrew/opt/node@22/bin/node cargo test -p server --test api collaboration::hocuspocus_two_clients_save_to_rust_and_reload_from_postgres -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
```
