# Báo cáo hoàn tất review fixes 11

Hoàn tất các mục P0, P1 và P2 của `11-review-fixes.md`. Mỗi mục có commit riêng; các mục qua hai repository có commit tương ứng ở từng repo. Commit bổ sung được gộp vào đúng mục trước khi chốt báo cáo. Các commit trong báo cáo là local, chưa push/deploy trong đợt này.

| Mục | Repo | Commit | Kết quả |
| --- | --- | --- | --- |
| Chỉnh plan | pathnote-rust | `9c139f4` | Đối chiếu code; thử QueryObserver disabled, sửa hướng xử lý P1.4 và các nhánh P0. |
| P0.1 | pathnote-rust | `7ef3d3c` | Dump trước migration; root-owned 600, khóa backup, giữ 5 bản, từ chối ID cũ và không ghi đè; lỗi/rỗng không chạy migration. |
| P0.2 | pathnote-rust | `bba579e` | Boolean manifest; stop → backup → migration → activate. Chưa gọi migration thì khôi phục service cũ; đã gọi thì giữ dừng, không rollback binary. |
| P0.3 | pathnote-rust | `01ded06` | Checklist locale và backup chỉ viết tài liệu. Người vận hành thực hiện trên production. |
| P1.1 | pathnote-rust | `2a25fff` | Index meanings, bỏ index trùng; test migration up/down; fixture theo tên migration và dọn schema. |
| P1.2 | pathnote-rust | `443f977` | Upstream dịch 20s; timeout JSON HTTP 504 và giữ message. B.6 đã cập nhật. |
| P1.2 | pathnote-web | `d43b896` | Client dịch 28s; test/build/format. |
| P1.3 | pathnote-web | `0c6fa54` | Xóa detail cache và recent persisted; dọn recent 403/404. Tests xóa session/course/place. |
| P1.4 | pathnote-web | `10868e7` | Catalog enabled theo đăng nhập; bỏ tải thủ công trùng. Test rename refetch đúng một lần. |
| P1.5 | pathnote-collaboration | `12189c1` | Startup mặc định Rust 3000/api/v1; cảnh báo legacy prefix; cập nhật env/template. |
| P1.6 | pathnote-collaboration | `5e992a1` | Pin SSH_KNOWN_HOSTS, bỏ keyscan; test và actionlint. |
| P1.6 | pathnote-pronunciation | `2c9d1d2` | Pin SSH_KNOWN_HOSTS, bỏ keyscan; test và actionlint. |
| P1.7 | pathnote-rust | `1ee9a62` | Manifest SHA collaboration; HTTPS /up, đúng SHA/status, từ chối redirect; kiểm tra trước upload và activate frontend. |
| P1.7 | pathnote-collaboration | `f3a2f13` | Thứ tự collaboration → backend → frontend và schema tương thích tab cũ. |
| P2.1 | pathnote-web | `656458c` | Đọc statistics cache, bỏ emit count/summary; test vào lại view có cache. Chỉ tải danh sách stats khi xem nơi học. |
| P2.2 | pathnote-web | `1b05b8b` | Không dùng placeholder của khóa trước khi chọn buổi mẫu. |
| P2.3 | pathnote-web | `6b12c7a` | Thêm invalidation notes/AI và generation guard; cập nhật bảng architecture, có tests. |
| P2.4 | pathnote-web | `2eb66ae` | Đổi session tăng version, xóa actionError. |
| P2.5 | pathnote-web | `acd5e5c` | Bỏ polling chết; giữ field syncing tương thích response. |
| P2.6 | pathnote-rust | `aeaa72d` | 404/405 có no-store/no-referrer và JSON, có test HTTP. |
| P2.7 | pathnote-rust | `ec6ac69` | Request ID trong trace span và expose CORS; test propagation/expose. |
| P2.8 | pathnote-rust | `ab9c0b0` | Giữ default predicate tower-http, thêm allowlist JSON/text; test PDF/ZIP/DOCX/image. |
| P2.9 | pathnote-rust | `07a6fa1` | Test Again và đổi bài nguồn: reviewed=2, unique=1. Ghi B.6. |
| P2.10 | pathnote-rust | `923a249` | auth/me qua service; ngoại lệ multi-place lock ở A.1 và comment. |
| P2.11 | pathnote-rust | `c84ecc5` | Test mảng <64KiB nhưng cả body >64KiB trả 422. |
| P2.12 | pathnote-collaboration | `0c68608` | Tests giữ socket khi 503/timeout, đóng khi 401/403/404. |
| P2.13 | pathnote-collaboration | `bded8db` | Retry failed document một lần; tổng shutdown 10s. Tests lưu cuối thật, lỗi và deadline. |
| P2.14 | pathnote-collaboration | `92d5259` | README dùng Rust; bỏ listener PHP cũ khỏi Nginx template (không sửa Nginx trên VPS). |
| P2.15 | pathnote-collaboration | `9652801` | Ignore .keys/. |
| P2.16 | pathnote-pronunciation | `0fed861` | Read timeout 10s, TasksMax=64; test socket gửi body dở bị đóng. |
| P2.17 | pathnote-pronunciation | `eaa3f57` | README gọi Rust, bỏ hướng dẫn artisan và đường dẫn cũ. |
| P2.18 | pathnote-pronunciation | `0222bfc` | Lock universal bằng uv 0.13.1; deploy pip --require-hashes, venv key từ lockfile; kiểm tra resolve/hash. |
| P2.19 | pathnote-pronunciation | `6440134` | Ignore .env.rust. |
| P2.20 | pathnote-web | `e285727` | Ignore .env, không bỏ .env.example. |
| Tài liệu điều phối | pathnote-web | `27c88db` | Commit phần hướng dẫn review fixes đã có trong AGENTS.md. |

## Kiểm tra cuối

- Rust: `cargo fmt --all --check`, clippy workspace/all-targets `-D warnings`, toàn bộ workspace tests đều pass. Nhóm API: **104 pass, 2 ignored** (hai integration tùy chọn cần Node bridge và dịch vụ IPA/dịch thật); các unit/integration khác pass. Đã chạy riêng integration hai client Hocuspocus → Rust → PostgreSQL: **1 pass**; integration cần dịch vụ IPA/dịch thật chưa chạy.
- Deploy: **34 test**, macOS **21 pass/13 skip**, Linux container **29 pass/5 skip**. Hai môi trường bổ sung nhau: fixture /opt/root chạy trong Linux; 5 test topology dùng Ruby chạy trên macOS. Actionlint và bash syntax pass.
- Frontend Node 22: **196 test pass**, build và format:check pass. Sau gộp commit đã so sánh tree, nội dung code không đổi so với bản vừa kiểm tra.
- Collaboration Node 22: **17 test pass**, gồm hai client WebSocket thật, lỗi quyền tạm/vĩnh viễn và lần lưu cuối.
- Pronunciation: **22 test pass**, gồm socket timeout; bash syntax và actionlint pass.
- Lock dịch: resolve require-hashes cho Linux x86_64/Python 3.11, 3.12, 3.13; pip thật dry-run require-hashes trong container Linux aarch64/Python 3.13 chấp nhận đủ 59 package. Không cài model dịch trong lượt kiểm tra này.

Dùng pg_dump/pg_restore chuẩn, predicate của tower-http và hooks lưu/destroy sẵn có của Hocuspocus. Code ứng dụng chỉ bổ sung quyền/retention backup, allowlist loại nội dung và lần retry failed document có giới hạn thời gian. Lockfile được tạo bằng [uv](https://docs.astral.sh/uv/pip/compile/), không tự tạo hash package.

## Người vận hành cần làm trước production

1. Theo [README VPS](../deploy/README.md#backup-bắt-buộc-trước-migration), cài client PostgreSQL đúng major, helper root-owned `/usr/local/sbin/pathnote-db-backup`, thư mục `/var/backups/pathnote-rust`, symlink `/opt/pathnote-rust/backups` và sudoers giới hạn. Nếu dùng đường dẫn/user khác mẫu, chỉnh các hằng số helper và cấu hình tương ứng.
2. Làm [checklist migration](../deploy/coordinated-release.md#checklist-người-vận-hành-lần-đầu-có-m20261016_) trên đúng DB production: kiểm tra locale, backup thử đọc được, sắp xếp dừng các writer. Giữ `breaking_migration: true` cho lần phát hành đang chọn. Sau khi migration bắt đầu, lỗi phải phục hồi DB có kiểm soát, không chỉ restart binary cũ.
3. Thêm `SSH_KNOWN_HOSTS` đã xác minh vào repo collaboration và pronunciation. Đảm bảo env collaboration thực tế dùng Rust URL/prefix `http://127.0.0.1:3000` + `/api/v1`, cùng secret với Rust; template mới không tự sửa env đã có trên VPS.
4. Cài lại unit pronunciation đã thêm TasksMax và chạy daemon-reload theo README. Workflow không tự ghi đè unit/system config đã cài.
5. Push/deploy collaboration theo SHA trong manifest trước. Xác nhận HTTPS `/up` trả đúng SHA, sau đó mới phát hành backend/frontend qua workflow Rust. Manifest cuối đã chọn HEAD frontend và collaboration có đầy đủ các fix trong báo cáo.

## Giới hạn thực tế

- Không chạy SQL, SSH, backup/restore hoặc phát hành trên production; chưa chạy workflow GitHub của các commit mới này. Health/version gate production chưa được xác minh.
- Fixture deploy dùng systemctl/migration/pg_dump giả; helper được kiểm tra quyền file, lỗi/rỗng, giữ 5 bản, chống ghi đè/ID cũ. Chưa thử phục hồi DB thật trên VPS.
- Shutdown chỉ thử lưu cuối một lần, không thay hàng đợi bền vững khi API mất kết nối lâu. Nếu vẫn lỗi hoặc quá 10 giây, process thoát 1 và báo có thể còn document chưa lưu.
- Hash lock bảo vệ các distribution runtime đã chọn. Một số package có source archive; toolchain build của pip/venv và các model tải khi khởi động chưa được khóa bởi lockfile này. Chưa chạy đầy đủ CLI/model trên VPS với venv mới.
- Giữ nguyên các mục được ghi rõ “Không làm” trong plan.
