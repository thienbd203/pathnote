# Flashcard và IPA/dịch tự động

## Phạm vi đã triển khai

- Gõ từ: nghĩa + IPA + chữ xáo trộn, nghe bằng giọng trình duyệt.
- Chọn từ: 2–4 phương án, khác từ/ nghĩa và cùng loại từ + giọng.
- Mỗi câu tối đa hai lần thử. Sai lần đầu chưa kết thúc câu; typing trả pattern
  để frontend dùng ô điền chữ còn thiếu. Sai lần hai kết thúc và hiện đáp án.
- Đúng ở bất kỳ lần nào: hiện từ, IPA, ví dụ và tự đọc; giữ phím A–D / 1–4,
  Enter và focus nút tiếp theo của giao diện đã có.
- Bộ câu hỏi và lần thử lưu JSONB, tiến độ lưu riêng theo `(user_id, card_id)`.
  Tải lại trang tiếp tục được. Retry request trùng không cộng điểm lại.
- Queue chưa cần cho phần lấy từ/ chấm đáp án. IPA/translation gọi dịch vụ hiện có
  theo request có timeout; không dùng fire-and-forget `tokio::spawn` để cập nhật note.

## Các module cần đọc để học Rust

`vocabulary/reviews/dto.rs`: enum `Mode` cho Serde, kiểu snapshot và kiểu request.
`create.rs`: SeaORM query, random sample, ưu tiên lỗi và tạo snapshot.
`state.rs`: kiểm tra quyền theo nguồn, kiểm tra chữ/nghĩa còn đúng phiên bản,
projection response không đưa snapshot đáp án trực tiếp xuống frontend.
`answer.rs`: transaction, khóa review/cha, state machine hai lần thử, idempotency,
upsert progress và thời điểm ôn lại.
`enrichment/client.rs`: reqwest client dùng lại connection pool, không forward
JWT/cookie, không follow redirect, timeout và giới hạn response.
`enrichment/handlers.rs`: kiểm tra quyền ghi chú trước khi gọi service.

Các test đều qua HTTP API trong `server/tests/api`, không viết unit test riêng
cho từng function service/helper.

## API flashcard

- POST `/api/v1/vocabulary-reviews`: `mode=typing|choice`, `limit` 1–50, các filter
  `session_id`, `topic`, `q` của kho từ. Mặc định 20 câu.
- GET `/api/v1/vocabulary-reviews/{id}`: chỉ chủ lượt ôn, tiếp tục lần đang làm.
- POST `/api/v1/vocabulary-reviews/{id}/answers`: `position`, `answer`, `attempt=0|1`.
- GET `/api/v1/vocabulary-reviews/{id}/pronunciation?position=...`: từ/locale của
  câu typing hiện tại để browser speech đọc. Đây là gợi ý học, không phải cơ chế chống gian lận.

Chỉ trả câu đang làm; kết quả trả card đầy đủ sau khi câu kết thúc. Không trả toàn bộ
questions JSONB hoặc answer key ở đầu lượt. Câu typing vẫn có IPA và chữ gợi ý;
choice có từ trong options theo yêu cầu sản phẩm.

Nguồn gồm cả bài của câu hỏi lẫn bài dùng tạo phương án. Khi mất quyền mở nguồn
hoặc nguồn bị xóa mềm, cả lượt ôn trả 404. Từ hoặc nghĩa đổi thì câu đang làm trả 409;
sửa IPA không hủy lượt, kết quả dùng IPA mới. Progress chỉ dùng nếu study_key vẫn
khớp word/locale/POS/meaning; đổi nghĩa không kéo theo số điểm cũ.

Đúng lần đầu tăng streak và đặt due_at 1/3/7/14/30 ngày. Cần lần hai hoặc sai
thì streak về 0, due_at sau 10 phút. Đây là lịch nhắc đơn giản, không phải FSRS.
Frontend hiện ưu tiên từ từng sai, chưa lọc chỉ từ đã đến hạn.

Tạo lượt lấy mẫu random tối đa 1000 candidate và 1000 distractor từ dữ liệu đúng quyền,
ưu tiên incorrect trong sample. Không phải giới hạn số từ trong kho. Chưa có benchmark
kho rất lớn hay thuật toán chọn deck tối ưu cho hàng triệu card.

## API IPA/dịch

- POST `/api/v1/sessions/{id}/pronunciation`: text <=1000 ký tự, locale US/UK.
- POST `/api/v1/sessions/{id}/pronunciation/batch`: 1–50 texts, <=64 KiB;
  dedup để gọi dịch vụ rồi trả đúng thứ tự/số mục request, hỗ trợ lỗi từng mục.
- POST `/api/v1/sessions/{id}/translation`: text <=1000 ký tự, EN → VI.

Chủ bài hoặc collaborator được gọi. Upstream sai cấu trúc, locale, thứ tự, lỗi
hoặc timeout thì trả 503 để người dùng nhập tay/thử lại. API không sửa note trực tiếp;
editor áp kết quả nếu field vẫn còn và chưa đổi, rồi lưu qua cơ chế note hiện có.

Rust `.env`:

```ini
PRONUNCIATION_URL=http://127.0.0.1:8020
TRANSLATION_URL=http://127.0.0.1:8022
```

Để trống URL thì không bật lookup tự động trong editor. Hai URL là cấu hình server,
không lấy từ body người dùng. Không forward credential tới dịch vụ.
Các service hiện có ở repo `pathnote-pronunciation`:

```sh
# pathnote-pronunciation
# Container IPA cũ có thể chưa có batch: cập nhật image từ server.py hiện tại.
docker compose up -d --build pronunciation
docker compose start translation
# pathnote-rust
cargo run -p migration -- up
cargo run -p server
```

Đã bật container local, cập nhật IPA batch, áp migration và đặt URL local. Rust cần
restart sau khi đổi .env (watchexec local đã reload). Production chưa thay đổi.
Chưa thêm cache dịch/IPA ở Rust; chưa triển khai dashboard worker hoặc queue package.

## Kiểm tra

58 test API thông thường qua; riêng test tích hợp yêu cầu service đã chạy:

```sh
cargo test --workspace
cargo test -p server --test api enrichment::enrichment_with_real_local_services -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
```

Các case: giấu snapshot đáp án, first-attempt/retry/finish, stale attempt, retry
trùng và đồng thời, tiến độ riêng, thay nghĩa/IPA, nguồn chia sẻ bị gỡ/xóa, phương án
không hợp lệ, lô IPA dedup/partial failure/upstream sai, JWT không forward.
Test eSpeak đơn/lô + LibreTranslate thật đã qua. Frontend 158 test và build qua.
Chưa kiểm tra thao tác UI bằng tài khoản Google thật trong bước này.

Nguồn: [reqwest ClientBuilder](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html),
[rand SliceRandom](https://docs.rs/rand/0.8.5/rand/seq/trait.SliceRandom.html).
