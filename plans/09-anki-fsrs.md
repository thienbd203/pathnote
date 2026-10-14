# Ôn flashcard theo FSRS

## Đã triển khai

- Kho từ dùng nút Ôn flashcard làm luồng chính. Gõ từ/chọn từ là luyện tập riêng,
  không suy diễn điểm đúng/sai cũ thành lịch sử FSRS.
- Mặt trước Việt → Anh, Anh → Việt hoặc cả hai. Cấu hình và retention lưu theo
  user + buổi học + chủ đề. Search chỉ lọc thẻ, không tạo cấu hình khác.
- Tiến độ theo `(user_id, card_id, direction)`, dùng chung khi mở thẻ đó từ bộ
  lọc khác. Mỗi nghĩa trong bảng nguồn là một card; cùng chữ ở hai bài là hai card.
- Ưu tiên quá hạn trước thẻ mới, xáo trộn thẻ mới. Không lấy mẫu giới hạn 1000
  như bài luyện tập. Thẻ có lịch tương lai chỉ xuất hiện lại khi đến hạn.
- Lật đáp án: từ, IPA, nghĩa và ví dụ. Tự đọc từ khi lật; có nút nghe và tốc độ đọc.
- Space/Enter lật; 1–4 đánh giá Quên/Khó/Nhớ/Dễ. Chấm xong tự chuyển thẻ.
- Learning dùng bước 1 phút/10 phút, Hard ở bước đầu là trung bình 5,5 phút;
  relearning một bước 10 phút. Màn hình hiển thị khoảng cách trước khi chấm.
- Thư viện `fsrs` 6.6.2 tính stability, difficulty và khoảng ôn dài hạn bằng
  FSRS-6. Retention mặc định 90%, chỉnh 70–97% qua API; UI có các mức phổ biến.
- Lưu trạng thái trí nhớ và lịch sử đánh giá trong PostgreSQL. Retry cùng
  attempt/rating trả kết quả đã lưu. Hai tab cùng học một card: chỉ lượt đầu
  được chấm, tab còn lại nhận 409. Lịch hai chiều và hai người không trộn nhau.
- Đổi word/meaning/locale/POS làm card bắt đầu lại. Đổi IPA/ví dụ/chủ đề không
  đổi trí nhớ. Xóa nguồn hoặc thu hồi quyền thì không xem/chấm thẻ cũ được.

## Phần thư viện và phần ứng dụng

`scheduler.rs` gọi `FSRS::next_states`, không tự viết công thức FSRS.
Ứng dụng quyết định learning steps, chọn thẻ, làm tròn ngày (tối thiểu 1, tối đa
36500 ngày), giao diện và bảo vệ cập nhật. Interval dài tính theo thời gian UTC,
elapsed là số chu kỳ 24 giờ đã qua; chưa áp dụng day cutoff/fuzz của Anki.
Dùng tham số mặc định khi chưa đủ lịch sử; optimizer tự cá nhân hóa khi đủ dữ liệu.
Chưa import/export `.apkg`, daily limit hoặc bury sibling như đầy đủ trong Anki.

## API

- GET `/api/v1/flashcards?session_id=...&topic=...&q=...`: settings, new, due,
  learning, thời điểm thẻ tương lai gần nhất.
- PUT `/api/v1/flashcards`: bộ lọc + direction `vi_en|en_vi|both` + retention.
- POST `/api/v1/flashcards/next`: bộ lọc; trả mặt trước và UUID attempt, hoặc
  `card:null` khi chưa có thẻ đến hạn/mới. Không tự đánh dấu đã học.
- POST `/api/v1/flashcards/{uuid}/reveal`: lật và lấy đáp án/khoảng ôn dự kiến.
- POST `/api/v1/flashcards/{uuid}/grade`: rating 1–4; bắt buộc lật trước,
  attempt hết hạn sau 24 giờ. Khoảng ôn tính tại lúc chấm.

Các thao tác vẫn kiểm tra quyền JWT/nguồn. Chấm khóa place và user trong
transaction để đồng bộ với sửa bài, thu hồi quyền và tab khác. Không giữ lock
khi gọi dịch/IPA/audio hay truy vấn toàn bộ queue. Audio dùng giọng trình duyệt.

## Học Rust

Đọc `vocabulary/flashcards/dto.rs` (enum Serde), `queue.rs` (left join, aggregate,
lọc theo quyền), `scheduler.rs` (sử dụng crate), `mod.rs` (HTTP/transaction).
Test qua API nằm ở `server/tests/api/flashcards.rs`.

```sh
cargo run -p migration -- up
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Tham khảo: [FSRS Rust](https://docs.rs/fsrs/6.6.2/fsrs/),
[Anki deck options](https://docs.ankiweb.net/deck-options.html).

## Tự tối ưu theo lịch sử cá nhân

Server Rust tự kiểm tra mỗi phút, không có nút hoặc POST tối ưu. Lịch sử chấm
trong PostgreSQL là nguồn công việc bền vững: restart không mất dữ liệu cần tối ưu.
SQLx `PgAdvisoryLock` đảm bảo chỉ một optimizer chạy trên cùng database; CPU chạy
qua `spawn_blocking`, không giữ transaction/row lock trong lúc huấn luyện.

- Ngưỡng của Pathnote là 64 mẫu ôn dài hạn hợp lệ, tương ứng ngưỡng full training
  của crate; không phải quy tắc chung rằng Anki cần đúng 64 lượt. Nhiều dữ liệu
  hơn sẽ giúp fit tốt hơn. Same-day reviews vẫn nằm trong chuỗi trí nhớ nhưng
  không được tính vào ngưỡng mẫu ôn dài hạn.
- Chuỗi tách theo user/card/direction/study key và lần bắt đầu lại; chỉ dùng chuỗi
  bắt đầu từ review 0, liên tục, đã chấm và có timestamp. Không lấy lượt lật chưa
  chấm, bài luyện typing/choice hoặc lịch sử người khác.
- `compute_parameters` của crate tự fit. So sánh log-loss **trên tập huấn luyện**
  với model hiện tại bằng `FSRS::evaluate`; chỉ thay tham số khi kết quả hợp lệ
  và loss thấp hơn. Đây chưa phải đánh giá chất lượng bằng tập kiểm định độc lập.
- Tham số chung theo từng người, tiến độ vẫn riêng mỗi chiều/card. Dữ liệu không
  gửi tới Google/OpenAI hay dịch vụ bên ngoài.
- Tự fit lại khi có thêm 64 mẫu, hoặc sau 30 ngày khi có dữ liệu mới. Ít nhất một
  giờ giữa các lần thử. Nếu lỗi/crash/timeout 90 giây, giữ model cũ và tự thử lại.
- Thẻ đang mở giữ snapshot tham số để khoảng ôn đã xem không thay đổi đột ngột.
  Thẻ mới lấy model mới; memory được replay bằng `FSRS::memory_state` khi phiên
  bản model thay đổi. Không đổi hàng loạt ngày đến hạn hiện tại.
- Tải tối đa 50.000 review logs mỗi người mỗi lượt tối ưu; quá mức này giữ lịch
  hiện có, không cắt chuỗi tùy tiện. Prefix huấn luyện tối đa 256 reviews theo
  giới hạn mặc định của crate. Đây là giới hạn xử lý optimizer, không giới hạn kho.

GET `/api/v1/flashcards/optimization` chỉ trả trạng thái tự động, số mẫu, mốc đủ
và thời điểm cập nhật. Frontend hiển thị trạng thái, không yêu cầu người dùng thao tác.

Test `fsrs_optimization.rs` tạo lịch sử trong schema riêng và chạy optimizer thật:
fit tham số, giữ due date, snapshot đang mở, replay memory theo model mới, retry/
restart không fit lại và cách ly người dùng. Không tạo lịch sử giả vào DB local.

## Xem lại lịch sử

Màn `/flashcards` có Ôn tập và Lịch sử; khi đang ôn có liên kết về Lịch sử.
GET `/api/v1/flashcards/history` phân trang mới nhất trước, lọc session/topic/q,
rating và date + timezone. Mốc kết thúc ngày là đầu ngày kế tiếp tại múi giờ đã
chọn, không cộng cứng 24 giờ. Chỉ trả lượt đã chấm của chính người gọi, vẫn kiểm
tra quyền nguồn; nguồn xóa mềm hoặc bị gỡ chia sẻ sẽ không hiển thị.

Mỗi lượt chấm mới lưu snapshot word/meaning/IPA/POS/topics/examples. Xem lại chỉ
GET và phát âm bằng trình duyệt, không cộng review hoặc thay đổi memory/due_at.
Nếu từ đã bị gỡ khỏi bảng nhưng bài học còn truy cập được, snapshot vẫn xem được.
Log cũ thiếu snapshot: chỉ dùng thẻ hiện tại nếu study key còn khớp, có ghi rõ trên
UI. Nếu nghĩa đã đổi thì card=null, không dựng lại đáp án cũ từ nội dung mới.
Lịch hiển thị là due_at đã được đặt **sau lượt đó**, không phải lịch hiện tại mới nhất.

Test `flashcard_history.rs` kiểm tra snapshot sau sửa/gỡ từ, replay chấm không
nhân lịch sử, GET không thay memory, phân trang, ngày DST 25 giờ, cách ly user,
thu hồi quyền và xóa nguồn.
