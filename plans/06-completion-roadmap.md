# Rà soát và kế hoạch hoàn thiện — 14/10/2026

## Phạm vi và bằng chứng

Đối chiếu code hiện tại của `pathnote-rust`, frontend nhánh `feat/rust-auth-flow`,
Hocuspocus và hành vi Laravel làm nguồn tham khảo. Không coi các component cũ hoặc
kế hoạch viết trước đây là tính năng Rust đã hoàn thành. Chưa đánh giá production
qua SSH; chưa xác nhận Google thật qua UI trong đợt rà soát này.

- Rust: 62 API test qua, 2 test external integration mặc định ignored; Clippy và
  fmt qua ở lượt sửa row lock ngay trước đợt rà soát. Hai integration thực từng
  chạy qua ở lượt triển khai trước, chưa chạy lại trong đợt rà soát này.
- Frontend: chạy lại 158 test, typecheck và production build đều qua.
  Build cảnh báo chunk NoteEditor khoảng 735 kB (gzip 222 kB); cần đo tải thực tế
  trước khi tách thêm chunk, chưa kết luận đây là nguyên nhân chậm.
- Sửa transaction và cleanup media đang ở working tree, chưa commit.
- Tài liệu `02a/02b/02c` là các bước học theo lịch sử. Trạng thái tổng thể lấy ở
  tài liệu này và `plans/README.md`; `pathnote-docs` chủ yếu mô tả bản Laravel cũ.

## Ma trận tính năng

| Luồng | Rust | Frontend | Phần còn lại |
| --- | --- | --- | --- |
| Google → session DB → JWT 15 phút → refresh/logout | API, JWKS, test Google giả lập và session persistence | Token trong memory, single-flight refresh, account/logout guards | Test Google thật, F5, nhiều tab, JWT hết hạn khi đang sửa |
| Places/Courses/Sessions | CRUD, scope 404, pagination/search, soft delete, bulk ba cấp | CRUD; tìm ngày và bulk sessions | UI bulk places/courses; UI tìm nơi/khóa nếu cần |
| Ghi chú | JSONB, typed validation, revision conflict, autosave | Rich editor, giữ draft khi lỗi | Browser test lỗi mạng, conflict, reload/logout khi chưa lưu |
| Chia sẻ | Owner quản lý, collaborator sửa note, list shared | Mời/gỡ và bài chia sẻ | Test hai tài khoản thật và thu hồi khi đang chỉnh sửa |
| Ảnh/tệp | Private bytes, content validation, references, compensation và orphan cleanup | Upload/paste/drop/replace/undo/download | Test UI + backup/restore metadata và bytes cùng nhau |
| Realtime | Bridge quyền và JSON/Yjs persistence | Hocuspocus/Yjs, presence, reconnect, draft guard | Google thật; JWT hết hạn; restart service; chỉ chạy một instance hiện tại |
| Kho từ | Extract/normalize trước khóa; apply cùng transaction; backfill | Bảng, import merge/trim, lọc theo bài/chủ đề | Đo thời gian lưu với bài lớn; chưa có số liệu cần queue |
| Flashcard | Typing/choice, hai lần thử, progress riêng, retry idempotent | IPA/audio, chữ xáo trộn/OTP retry, phím tắt | Browser test đủ các luồng; due_at có lưu nhưng chưa có chế độ chỉ ôn đến hạn |
| IPA/dịch trong bảng từ | API đơn/lô và translation qua service có sẵn | Bảng từ đã nối Rust | Trang IPA riêng và block phát âm cũ chưa hoàn thiện |
| Bài tập từng node | Chưa có module/entity/API; Node enum chưa nhận exercise | UI và JSON parser cũ có sẵn nhưng basic editor ẩn tính năng | Ưu tiên tiếp theo |
| Học cùng AI | Nội dung thường/từ vựng có thể dùng note API; exercise còn thiếu | Dialog copy full-page context/import đã có nhưng đang ẩn | Nối Rust và đảm bảo import nguyên tử |
| Deploy Rust | Chưa có `.github` workflow trong repo Rust | Workflow web hiện chỉ deploy master | CI Rust, staging, systemd/reverse proxy, release/rollback và cutover dữ liệu |

## Cập nhật triển khai 6a

Đã nối API và frontend choice/fill, migration local và test realtime hai client.
Xem [hướng dẫn và phạm vi 6a](06a-inline-exercises.md). 66 API test + integration
Hocuspocus riêng, 158 frontend test/build và Clippy qua. Smoke UI với Google thật
vẫn chưa làm; không đánh dấu hoàn thành mục browser.
Ma trận phía trên là baseline lúc rà soát, cập nhật 6a này là trạng thái mới hơn.

## Giai đoạn 6 — Bài tập ngay trong ghi chú

Mục tiêu: giữ UI hiện tại; **mỗi câu hỏi là một node riêng**, không biến thành
wizard “câu tiếp theo”. Đáp án lưu server, chỉ trả cho người học khi câu kết thúc;
API sửa đề có quyền riêng. Đây là luyện tập, không đặt mục tiêu chống gian lận thi cử.

### 6a. Luồng đầu tiên: chọn đáp án và điền từ

- [x] Module `exercises`: DTO enum tagged Serde cho question/answer, service và
  handler; SeaQuery migration/entity cho đề và lượt làm từng người theo revision.
  Node chỉ chứa exercise ID; đề/đáp án ở bảng riêng, kết quả không nằm trong Yjs chung.
- [x] API tạo/import, đọc đề public, đọc/sửa definition có quyền, bắt đầu/tiếp tục/
  làm lại lượt và trả lời. Prefix `/api/v1`; response dùng data/HTTP status hiện có.
- [x] Quyền: owner và người tạo đề đang còn quyền sửa note được quản lý đề;
  người có quyền note được làm bài. Thiếu quyền mở note/cha bị xóa trả 404;
  không có quyền quản lý definition không trả đáp án. Test luật này trước khi nối UI.
- [x] Mỗi câu tối đa hai lần thử. Sai lần đầu giữ lượt; đúng hoặc sai lần hai kết
  thúc và hiện đáp án/giải thích. Revision cũ trả 409; request retry không tính lại.
- [x] Thêm Node::Exercise và validation reference thuộc đúng bài ở REST lẫn bridge.
  Validate cấu trúc ngoài transaction; quyền/reference/revision kiểm tra trong khóa.
- [x] Frontend chuyển transport bài tập sang Rust; chỉ mở command khi backend và
  bridge đã hỗ trợ. Không bỏ `basic` hàng loạt làm lộ chức năng chưa nối.
- [x] Test API: tạo → chèn node → lưu → đọc public → trả lời → reload → sửa đề →
  conflict; người khác/gỡ quyền/xóa cha, đáp án không lộ, retry đồng thời.
- [ ] Test hai người qua browser: thấy chung đề, kết quả riêng; F5 giữ tiến độ, Ctrl+Z node hoạt động.

**Hoàn thành 6a:** chọn/điền chạy qua Rust cả autosave và realtime, không gọi API Laravel.

### 6b. Các dạng còn lại

- [x] Thêm classify, matching, ordering, translation, correction vào enum và grader.
  Luật từng dạng rõ ràng; dịch/sửa lỗi so đáp án mẫu, không tự thêm AI chấm ngữ nghĩa.
- [ ] Giữ UI đã có, hỗ trợ keyboard/focus và mobile. Test qua API theo từng dạng.
- [x] Không thêm API split legacy nếu chưa có yêu cầu nhập đề cũ nhiều câu vào Rust;
  bản mới luôn một câu/node. Không sao chép toàn bộ service Laravel.

**Học:** enum cho dữ liệu khác hình dạng, JSONB snapshot, projection giấu đáp án,
transaction/state machine và API integration tests.

## Giai đoạn 7 — Học cùng AI

- [x] Sao chép context toàn page: chủ đề, text, bảng từ và đề public; không kèm
  token, URL media private hoặc đáp án người học chưa được xem.
- [x] Giữ luồng copy prompt → ChatGPT → dán JSON, chưa cần API/SDK AI hoặc billing.
- [x] Preview đầy đủ và báo lỗi trước khi ghi. Nội dung rỗng có hướng dẫn nhập chủ đề,
  không tự đoán bài học.
- [x] Import tạo mỗi câu một resource/node, append section ở dưới; không ghi đè
  bài hiện có và áp dụng trim parser hiện tại; bảng từ là bảng bổ sung mới, không merge vào bảng cũ.
- [x] Thiết kế một operation import: cùng transaction tạo resources và cập nhật
  note (JSON/Yjs cần phối hợp với bridge đang có). Nếu chưa đảm bảo atomic với
  realtime, chốt cơ chế retry/idempotency và cleanup resources chưa tham chiếu
  trước khi mở UI. Không chỉ POST tạo đề rồi hy vọng autosave sẽ thành công.
- [x] Test API/retry/reference/rollback/cleanup và hai client realtime với marker giữ qua restart.
- [ ] Smoke đầy đủ dialog/undo/mất mạng qua browser với Google thật.

**Hoàn thành:** copy đúng bài đang mở, import text/từ/bài tập qua Rust và mở lại được.
**Học:** DTO import typed, chuyển dữ liệu thành node, atomicity giữa resource và document.

## Giai đoạn 8 — Khép kín frontend

- [x] UI chọn nhiều places/courses: dùng bulk API đã có, confirmation phạm vi con,
  lỗi không xóa một phần, reset cache/sidebar/selection sau thành công.
- [x] IPA: khôi phục route/nav của trang IPA nếu giữ tính năng từ bản cũ; nối kho
  IPA/minimal pairs/import với Rust. Tài nguyên IPA tĩnh hiện có tiếp tục dùng lại.
- [x] Block pronunciation: `EditorPronunciationView.vue` vẫn gọi `/api/sessions/...`
  và `pronunciation-flashcards` cũ; basic mode chưa gắn NodeView này. Chuyển
  lookup sang enrichment Rust và chốt cách lưu thẻ thủ công trước khi mở lại.
- [ ] Bỏ type `InitialSchedule` và dữ liệu/nhãn generator còn sót khi không còn
  nơi dùng. Không triển khai lại sinh lịch tự động.
- [ ] Rà callsites `/api/` legacy. Xóa code không dùng sau khi đã khớp từng tính năng,
  không xóa parser/UI còn cần cho bài tập và AI. Cân nhắc đổi tên LegacySessionNote
  cho đúng: hiện là REST autosave Rust, tên legacy dễ gây hiểu nhầm.
- [ ] Tùy chọn sau MVP: filter “đến hạn” cho flashcard. Không coi cache enrichment,
  thuật toán SRS nâng cao hoặc queue là điều kiện hoàn thành khi chưa có nhu cầu đo được.

## Giai đoạn 9 — Kiểm tra thật và chuẩn bị release

- [ ] Browser smoke với hai tài khoản Google: login/F5/refresh/logout; hierarchy;
  chia sẻ/revoke; media; hai tab editor; từ vựng/review; bài tập/AI.
- [ ] JWT hết 15 phút trong WebSocket, network disconnect/reconnect, DB/service
  restart và lưu lại đúng trạng thái. Frontend hiện refresh khi API 401/reconnect,
  cần xác nhận luồng này bằng test thật thay vì suy ra từ watch accessToken.
- [ ] Đo save latency và thời gian giữ lock với note lớn/131+ từ và hai người sửa.
  Đồng bộ từ hiện cùng transaction; chỉ đánh giá queue/cache khi dữ liệu đo cho thấy cần.
- [ ] CI Rust: fmt, Clippy, API tests với PostgreSQL riêng; test bridge Node22 và
  build/test web trong repo tương ứng. Không cần test helper/service riêng hàng loạt.
- [ ] Staging Rust: systemd tự restart, PostgreSQL, media/key ngoài release,
  HTTPS/WSS, Google callback, secrets bridge, upstream IPA/dịch, healthcheck.
  Thêm release workflow/migrate/health/rollback; frontend vẫn chưa merge master.
- [ ] Diễn tập backup/restore DB + media + Yjs state; xác định mapping user Google,
  ID bài/media/exercise và dữ liệu từ Laravel nếu chuyển bản đang dùng. Có dry-run
  và kiểm đếm, không đổi production chỉ vì local test đã qua.
- [ ] Chốt cutover/rollback sau smoke staging; việc deploy và chuyển dữ liệu là bước riêng.

## Phần chủ động để sau

Sinh lịch tự động đã bỏ. Không viết lại CRDT/Hocuspocus bằng Rust. Chưa cần Redis
coordination nhiều Hocuspocus instance, queue framework, key rotation nhiều khóa,
version history hoặc hard-delete media vì chưa có yêu cầu vận hành cụ thể. Cleanup
media hiện chỉ dọn file không có metadata, không xóa file có record nhưng mất node.

**Cập nhật 6b:** năm dạng còn lại đã nối, 69 API test/158 frontend test/build/Clippy
qua. Case mới: cặp sai/trùng/không đủ, câu dùng từ trùng, thiếu/thừa từ, đáp án mẫu
thay thế và caseSensitive. Smoke keyboard/mobile qua browser vẫn chưa làm.

**Cập nhật giai đoạn 7:** đã nối [Học cùng AI](07-ai-study.md), 72 API test +
Hocuspocus integration, 159 web test/build và 10 collaboration test qua.

**Bước kế tiếp:** giai đoạn 8 — IPA, block phát âm và bulk UI.

Cập nhật giai đoạn 8: [IPA và bulk UI](08-ipa-and-bulk-ui.md) đã nối; 74 API
test + upstream thật, 163 frontend test/build và Clippy qua. Smoke browser/staging
và CI/CD Rust vẫn chưa hoàn thành.
