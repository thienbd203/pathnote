# Trang IPA, block phát âm và UI xóa nhiều

## Trạng thái và cách thử

Đã mở route `/ipa` và mục **IPA & Từ vựng** ở navigation. Giữ bảng 44 âm, audio
mẫu UK, giọng browser US/UK, minimal pairs, tìm kiếm, mở ví dụ và import JSON.
Kho riêng mỗi user/giọng, không dùng token Rust gọi endpoint Laravel.

Trong ghi chú: `/` → **Phát âm**. Nhập từ/cụm từ, chọn US/UK, tạo IPA, dịch hoặc
nhập tay. Nghe bằng giọng trình duyệt. Lookup tự động dùng debounce/cancel và
không áp kết quả nếu text/locale/field đã đổi; không tự lookup khi enrichment tắt.

**Thêm vào từ vựng** thêm từ vào bảng của bài cùng giọng, tạo bảng khi chưa có.
Dùng merge hiện có để giữ UUID/nội dung/ví dụ đã nhập; không tạo thêm kho thẻ riêng.
Nếu đã có từ, giữ loại từ hiện có; từ mới chưa biết loại dùng other/phrase. Không
bịa câu ví dụ: tạo ô ví dụ trống cho người dùng bổ sung. Sửa/undo đi qua transaction
editor và autosave/Yjs, kho từ/flashcard đồng bộ khi note lưu thành công.
**Ôn tập** mở kho từ lọc theo buổi học; không dùng modal flashcards Laravel cũ.
Trạng thái đã thêm kiểm tra tài liệu hiện tại để không báo sai sau undo.

Tại `/places` hoặc trong phần Khóa học của `/places/{id}`, chọn **Chọn nhiều**,
checkbox từng mục hoặc chọn các mục đã tải (tối đa 100 mỗi lượt), rồi **Xóa**.
Dialog xác nhận rõ các cấp con cũng bị xóa. Hủy/đổi account xóa selection; lỗi API
không xóa cache một phần. Thành công invalidates cache hierarchy/session và reload
trang/sidebar, không giữ course/session của các parent đã xóa.

## Rust IPA API và schema

`ipa_library` lưu từng word/pair JSONB, unique `(user_id,locale,kind,key)`.
Không dùng một JSONB collection khổng lồ và không áp giới hạn tổng 1000 từ/cặp.
API GET trả trang 100 rows; frontend đọc các trang để giữ local search/pagination
UI hiện có. Với kho lớn, frontend vẫn tải toàn bộ dữ liệu user/giọng vào memory;
chưa tuyên bố hỗ trợ millions entries. Có thể chuyển tìm kiếm sang server khi cần.

| API (Bearer JWT) | Payload/kết quả |
| --- | --- |
| GET `/api/v1/ipa-vocabulary?locale=en-US&page=1` | data.version/locale/entries + meta |
| GET `/api/v1/ipa-pairs?locale=en-GB&page=1` | data.version/locale/sets + meta; một set có thể qua nhiều trang |
| POST `/api/v1/ipa-vocabulary/import` | vocabulary JSON v1; meta.added/skipped |
| POST `/api/v1/ipa-pairs/import` | ipa-pairs JSON v1; meta.added/skipped |

Import request tối đa 256 KiB; pairs 1–50 mỗi request, không phải giới hạn kho.
Từ trim/collapse whitespace/lowercase để dedup; US và UK tách riêng.
Typed Entry/Sense/Example dùng lại enum locale/POS và validator node từ vựng.
Cặp âm phải thuộc bảng 44 âm, IPA hai từ khác đúng một phoneme ở cùng vị trí.
Longest match nhận âm ghép; alias US normalize về âm tham chiếu để so sánh.
Cặp nhập đảo sounds/left/right vẫn là cùng identity; lưu orientation canonical,
frontend ghép lại theo tab. Batch có một cặp sai trả 422, không lưu một phần.

Chuẩn bị/validate JSON và phonemes ngoài transaction. Trong transaction khóa row
user để serialize imports, query các identity đang nhập, insert theo lô 100;
unique DB chống trùng. Đây là nghiệp vụ kho IPA, không thêm framework/queue/parser
JSON thay thế Serde. Chưa có API edit/delete kho IPA (UI cũ cũng không có).

## Frontend và bulk

- `IpaView.vue` + `lib/ipa-library.ts`: bounded paged transport, merge sets qua trang,
  refresh collection sau import; guard account và abort khi đổi locale/unmount.
- `EditorPronunciationView.vue`, `pronunciation-vocabulary.ts`: Rust enrichment,
  note-backed vocabulary, current-document saved status, route ôn theo bài.
- `vocabulary-json.ts::applyVocabularyContent`: dùng cùng incremental merge cho
  JSON import và từ block phát âm; giữ node identities cho Yjs, không replace cả table.
- `LearningBulkList.vue`: reusable selection/confirmation cho places/courses;
  `learning.ts::removeMany` gọi collection DELETE với ids, reset cache sau thành công.
- Backend bulk đã có quyền 404/transaction/cascade; không thêm controller clone.

## Kiểm tra

74 API test qua; IPA/dịch qua eSpeak + LibreTranslate thật đã chạy lại qua.
Case kho IPA: 131 từ, trang 100/31, user/locale scope, duplicate/case/whitespace và
imports đồng thời; pairs đảo chiều, alias US, batch sai không ghi một phần.
163 frontend test qua, typecheck/build qua, Clippy/fmt sạch. Case client mới:
paged collections, set qua nhiều trang; thêm từ giữ pronunciation block/UUID,
merge/undo; bulk transport, reset cache sau thành công, lỗi giữ dữ liệu.
Chưa smoke browser với Google thật trong đợt này; API/test state không thay cho
kiểm tra touch/keyboard trên mobile. Không chuyển dữ liệu IPA production cũ.

Migration đã áp vào PostgreSQL local. Chưa commit hoặc deploy production.

```sh
cargo run -p migration -- up
cargo test --workspace
cargo test -p server --test api enrichment::enrichment_with_real_local_services -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
```
