# Gộp Flashcard theo từ và loại từ

Chốt theo trao đổi ngày 16/10/2026: cùng từ, cùng loại từ dùng một lịch ôn. Các nghĩa ở nhiều bài được gom để hiển thị, không yêu cầu chuỗi nghĩa phải giống nhau.

Ví dụ: `remember` (verb) ở bài 1 có nghĩa “nhớ”, bài 2 có nghĩa “ghi nhớ” → một mục ôn, hiển thị cả hai nghĩa. `light` (noun) và `light` (adjective) → hai mục ôn, hai lịch riêng. Các locale vẫn tách riêng để giữ cấu hình ngôn ngữ/phát âm hiện tại.

## Hành vi

- Khóa lịch ôn là tài khoản + từ đã chuẩn hóa hoa/thường/khoảng trắng + locale + loại từ + chiều ôn. Nội dung và ID của từ trong từng ghi chú vẫn giữ nguyên.
- Các nghĩa đang có quyền truy cập được gom thành chuỗi phân cách bằng `; `. Các nghĩa giống nhau sau chuẩn hóa chỉ xuất hiện một lần. Đây là gom theo từ/loại từ, không phải AI tự suy đoán nghĩa tương đương.
- Nhập lại từ ở buổi khác hoặc đổi cách diễn đạt nghĩa không tạo lịch ôn mới. Khác loại từ hoặc locale vẫn tách riêng.
- Kiểm tra nội dung ticket tách khỏi khóa lịch ôn: sửa từ/nghĩa của nguồn làm ticket cũ bị từ chối như trước.
- Đánh giá được khóa theo tài khoản để hai tab mở hai bản sao không ghi hai lượt ôn chồng nhau. Chiều Anh–Việt và Việt–Anh vẫn độc lập.
- Lưu snapshot các nghĩa đã hiển thị khi xem đáp án; lịch sử không tự đổi theo cách ghi nghĩa về sau.
- Memory và chuỗi đánh giá không bị xóa dây chuyền khi xóa một nguồn, vì từ có thể còn ở bài khác. API lịch sử vẫn lọc quyền của nguồn; dữ liệu tài khoản vẫn bị xóa dây chuyền theo user.

## Nâng cấp dữ liệu

Hai migration chuyển khóa memory từ từng card nguồn sang nhóm từ. Khi các bản sao đã có lịch khác nhau, giữ trạng thái được ôn gần nhất (hòa thời điểm thì ưu tiên số lượt ôn, rồi ID nguồn). Giữ nguyên các attempt và định danh chuỗi lịch sử gốc để FSRS có thể tái dựng sau khi cá nhân hóa tham số.

Ticket có thêm hash nội dung nguồn và ID chuỗi memory. Các attempt cũ được backfill; ưu tiên danh tính ghi trong memory cũ nếu nguồn đã được sửa. Hash của nhóm dùng cùng cách mã hóa với Rust, có kiểm thử nâng cấp cả khoảng trắng Unicode.

Migration chỉ hỗ trợ nâng cấp tiến, không tự tách ngược lịch ôn đã gộp. Chưa triển khai production.

## Contract và frontend

- `GET /flashcards` và overview của `POST /flashcards/next`: `new`, `due`, `learning`, `reviewable` tính theo nhóm từ/loại từ thay vì từng nghĩa ở từng nguồn; `word_count` vẫn đếm từ đã chuẩn hóa.
- `front` của chiều Việt–Anh và `back.meaning` khi reveal có thể chứa nhiều nghĩa, phân cách bằng `; `. Shape JSON không đổi.
- Nhãn thống kê đổi từ “Nghĩa” sang “Mục ôn”; mô tả ôn hai chiều cũng dùng số mục ôn để tránh nhầm với tổng số nghĩa.

## Kiểm thử

- Cùng từ ở nhiều bài, khác cách viết hoa hoặc khác cách ghi nghĩa: gom đúng, giữ lịch, thêm nguồn không làm thành thẻ mới.
- Khác loại từ/locale: vẫn tách lịch.
- Không gom nghĩa riêng của tài khoản khác; lịch ôn riêng theo tài khoản.
- Hai ticket trùng từ ở hai bài đánh giá đồng thời: chỉ một lượt được ghi.
- Xóa nguồn ban đầu: nguồn còn lại vẫn dùng lịch đã học.
- Nâng cấp từ schema cũ: giữ lịch gần nhất và tái dựng đúng chuỗi lịch sử dù nguồn đã đổi từ; nhận diện khoảng trắng Unicode và khoảng trắng lặp.
- Kiểm thử lịch sử, thay đổi nội dung, quyền chia sẻ và cá nhân hóa FSRS hiện có vẫn được chạy cùng bộ kiểm thử.

Kết quả: toàn bộ workspace test qua; nhóm API 100 bài qua, 2 bài được đánh dấu bỏ qua. Clippy không cảnh báo và kiểm tra định dạng Rust qua. Frontend 190 bài qua, build và Prettier check qua. Chưa kiểm tra tương tác gộp bằng trình duyệt trên dữ liệu người dùng; xác nhận hành vi qua HTTP với PostgreSQL trong schema kiểm thử riêng.
