# Tiến độ review fixes 11

| Mục | Commit | Kiểm tra |
| --- | --- | --- |
| Chỉnh plan | 9c139f4 | Đối chiếu code và thử disabled QueryObserver; refetchType all không tải lại |
| P0.1 | 2e31a8a | Rust fmt/clippy; workspace tests (100 API pass, 2 ignored); deploy tests macOS 17 pass/8 skip, Linux container 20 pass/5 skip; bash syntax |

| P0.2 | 9e187dd | Rust fmt/clippy/workspace; macOS 18 pass/13 skip, Linux 26 pass/5 skip; actionlint; migration lỗi và health lỗi đều không rollback code cũ |

| P0.3 | Commit `Document operator checks before flashcard migrations` | Checklist chỉ viết tài liệu; không chạy SQL/backup production |

## Giới hạn và việc người vận hành cần làm

- Chưa chạy lệnh nào trên VPS/DB production; chưa kiểm chứng backup/restore production.
- P0.1: cài client PostgreSQL đúng major, helper root-owned và sudoers theo deploy/README.md trước deploy tiếp theo. Backup dùng pg_dump chuẩn, không tự viết định dạng backup. Thư mục thật nằm ngoài cây deploy-owned, /opt/.../backups là symlink tới /var/backups/pathnote-rust.
- Linux fixture dùng service/migration/pg_dump giả; helper test kiểm tra lỗi/rỗng/quyền file/retention/không ghi đè. Workflow parser tests chạy trên macOS vì container không có Ruby.
- Các mục còn lại chưa hoàn tất; không coi báo cáo này là xác nhận đủ điều kiện phát hành.
