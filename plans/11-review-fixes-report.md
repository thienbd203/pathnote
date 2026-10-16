# Tiến độ review fixes 11

| Mục | Commit | Kiểm tra |
| --- | --- | --- |
| Chỉnh plan | 9c139f4 | Đối chiếu code và thử disabled QueryObserver; refetchType all không tải lại |
| P0.1 | 2e31a8a | Rust fmt/clippy; workspace tests (100 API pass, 2 ignored); deploy tests macOS 17 pass/8 skip, Linux container 20 pass/5 skip; bash syntax |

| P0.2 | 9e187dd | Rust fmt/clippy/workspace; macOS 18 pass/13 skip, Linux 26 pass/5 skip; actionlint; migration lỗi và health lỗi đều không rollback code cũ |

| P0.3 | 8d69f4c | Checklist chỉ viết tài liệu; không chạy SQL/backup production |

| P1.1 | f58db37 | Migration up/down test; grouping fixture chọn migration theo tên; Rust fmt/clippy/workspace (101 API pass, 2 ignored) |

| P1.2 | Rust: 1ee8c53; Web: d43b896 | Rust fmt/clippy/workspace, test JSON 504; web 190 tests/build/format |

| P1.3 | Web: 0c6fa54 | 193 tests/build/format; xóa session/course/place dọn detail và recent persisted |
| P1.4 | Web: 10868e7 | 194 tests/build/format; rename refetch catalog đúng một lần |
| P1.5 | Collaboration: 12189c1 | 10 tests; default startup Rust 3000/api/v1 |
| P1.6 | Collaboration: 5e992a1; Pronunciation: 2c9d1d2 | npm test; Python tests; actionlint cả hai workflow |
| P1.7 | Rust: 42f9bc5; collaboration: f3a2f13 | Manifest/health/topology tests; actionlint; Rust fmt/clippy/workspace; collaboration 10 tests |

| P2.1 | Web: 0e6feee | 195 tests/build/format; số đếm có ngay khi tạo lại view từ cache |
| P2.2 | Web: f1408ec | 195 tests/build/format; không giữ placeholder của khóa trước |
| P2.3 | Web: c8c19aa | 196 tests/build/format; kiểm tra invalidation notes và AI import |
| P2.4 | Web: 0bda4ee | 196 tests/build/format; reset generation/error khi đổi lesson |
| P2.5 | Web: b7fdc2e | 196 tests/build/format; bỏ polling chết |
| P2.6 | Rust: commit `Apply browser headers to JSON fallback errors` | fmt/clippy/workspace; 404 và 405 đều có no-store/no-referrer và JSON |

## Giới hạn và việc người vận hành cần làm

- Chưa chạy lệnh nào trên VPS/DB production; chưa kiểm chứng backup/restore production.
- P0.1: cài client PostgreSQL đúng major, helper root-owned và sudoers theo deploy/README.md trước deploy tiếp theo. Backup dùng pg_dump chuẩn, không tự viết định dạng backup. Thư mục thật nằm ngoài cây deploy-owned, /opt/.../backups là symlink tới /var/backups/pathnote-rust.
- Linux fixture dùng service/migration/pg_dump giả; helper test kiểm tra lỗi/rỗng/quyền file/retention/không ghi đè. Workflow parser tests chạy trên macOS vì container không có Ruby.
- Trước deploy collaboration/pronunciation: thêm SSH_KNOWN_HOSTS đã xác minh vào secret từng repo. Gate collaboration cần phiên bản trong manifest được deploy trước.
- Các mục còn lại chưa hoàn tất; không coi báo cáo này là xác nhận đủ điều kiện phát hành.
