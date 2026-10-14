# Cấu hình VPS lần đầu và CI/CD Rust

Rust dùng database mới `pathnote_rust`; không chuyển dữ liệu Laravel. GitHub Actions
chạy test, build hai binary Linux rồi upload bằng SSH. VPS không cần Rust/Cargo.
Push lên `master` hoặc chạy workflow thủ công sẽ deploy sau khi cấu hình bên dưới.

## 1. Kiểm tra VPS và cài PostgreSQL

Các lệnh trên VPS dưới đây dành cho Ubuntu/Debian, chạy bằng root hoặc sudo.
Kiểm tra hệ điều hành và kiến trúc trước:

```sh
cat /etc/os-release
uname -m
sudo apt update
sudo apt install -y postgresql postgresql-contrib rsync python3 util-linux openssl ca-certificates nginx certbot python3-certbot-nginx
sudo systemctl enable --now postgresql
pg_lsclusters
sudo -u postgres psql -c 'SHOW listen_addresses;'
sudo -u postgres psql -c 'SHOW port;'
```

Giữ PostgreSQL chỉ nghe localhost, không mở cổng 5432 ra Internet. Nếu cấu hình
khác localhost, sửa `listen_addresses = 'localhost'` trong file `postgresql.conf`
của cluster rồi restart cluster. Dùng bản PostgreSQL đang được Ubuntu hỗ trợ;
[hướng dẫn PostgreSQL chính thức](https://www.postgresql.org/download/linux/ubuntu/).

Tạo tài khoản và database riêng, nhập password bằng prompt thay vì ghi vào history:

```sh
sudo -u postgres psql
```

Trong psql:

```sql
CREATE ROLE pathnote LOGIN;
\password pathnote
CREATE DATABASE pathnote_rust OWNER pathnote;
\q
```

Kiểm tra kết nối TCP (nhập password khi được hỏi):

```sh
psql -h 127.0.0.1 -U pathnote -d pathnote_rust -W -c 'SELECT current_database();'
```

Role này không cần SUPERUSER. Database owner đủ chạy migration nghiệp vụ và
hàm tạo bảng phiên của package.

## 2. User, thư mục và khóa JWT

User `pathnote` chạy ứng dụng; user `deploy` nhận artifact từ GitHub. Nếu đã có
user deploy thì dùng lại, điều chỉnh tên tương ứng trong sudoers và SSH_USER.

```sh
sudo useradd --system --user-group --no-create-home --shell /usr/sbin/nologin pathnote
sudo adduser --disabled-password --gecos '' deploy
sudo usermod -aG pathnote deploy
sudo install -d -o deploy -g pathnote -m 2750 /opt/pathnote-rust /opt/pathnote-rust/releases
sudo install -d -o root -g pathnote -m 750 /opt/pathnote-rust/shared /opt/pathnote-rust/shared/keys
sudo install -d -o pathnote -g pathnote -m 750 /opt/pathnote-rust/shared/media
```

Thêm public key riêng cho CI vào `/home/deploy/.ssh/authorized_keys`, thư mục
700 và file 600, owner deploy. Private key tương ứng chỉ đưa vào GitHub Secret.
Đăng nhập SSH lại để nhóm bổ sung có hiệu lực.

Tạo khóa JWT **một lần**; không ghi đè khóa khi deploy:

```sh
sudo sh -c 'test ! -e /opt/pathnote-rust/shared/keys/access-token.pem && umask 027 && openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 -out /opt/pathnote-rust/shared/keys/access-token.pem'
sudo chown root:pathnote /opt/pathnote-rust/shared/keys/access-token.pem
sudo chmod 640 /opt/pathnote-rust/shared/keys/access-token.pem
```

## 3. Upload mẫu cấu hình

Từ máy Mac, trong repo Rust (thay VPS_HOST và SSH port nếu khác 22):

```sh
scp deploy/production.env.example deploy/pathnote-rust.service deploy/nginx.conf.example root@VPS_HOST:/tmp/
```

Trên VPS:

```sh
sudo install -o root -g pathnote -m 640 /tmp/production.env.example /opt/pathnote-rust/shared/.env
sudo nano /opt/pathnote-rust/shared/.env
sudo install -m 644 /tmp/pathnote-rust.service /etc/systemd/system/pathnote-rust.service
sudo systemctl daemon-reload
sudo systemctl enable pathnote-rust.service
```

Chưa start service: binary sẽ được đưa lên ở lần deploy đầu. Điền DATABASE_URL,
Google client ID/secret, FRONTEND_URL, GOOGLE_REDIRECT_URL và JWT_ISSUER theo domain
thực tế. Password trong DATABASE_URL cần URL-encode ký tự đặc biệt (ví dụ `@` thành
`%40`). File env do dotenv/systemd đọc, không dùng `source` trong shell.

Giữ `APP_BIND=127.0.0.1:3000` và `COOKIE_SECURE=true`. Thêm callback Rust vào
Authorized redirect URIs trên Google Console; có thể dùng cùng OAuth client với
Laravel. Giữ khóa JWT và MEDIA_ROOT ngoài releases.

Nếu dịch/IPA chưa chạy, để trống TRANSLATION_URL/PRONUNCIATION_URL. Khi đã có
service trên VPS, dùng địa chỉ localhost trong mẫu. Realtime cần COLLABORATION_URL
WSS và cùng COLLABORATION_SECRET giữa Rust/Hocuspocus; Hocuspocus phải trỏ API nội
bộ sang Rust. Không tự đổi cấu hình Laravel đang hoạt động.

Cho deploy quyền cần thiết, mở bằng `sudo visudo -f /etc/sudoers.d/pathnote-rust`:

```sudoers
deploy ALL=(root) NOPASSWD: /usr/bin/systemctl restart pathnote-rust.service, /usr/bin/systemctl stop pathnote-rust.service
deploy ALL=(pathnote) NOPASSWD: /usr/bin/test -r /opt/pathnote-rust/shared/.env, /usr/bin/test -r /opt/pathnote-rust/shared/keys/access-token.pem, /usr/bin/test -w /opt/pathnote-rust/shared/media, /opt/pathnote-rust/releases/*/migration up
```

```sh
sudo chmod 440 /etc/sudoers.d/pathnote-rust
sudo visudo -cf /etc/sudoers.d/pathnote-rust
```

## 4. Domain và Nginx HTTPS

Chọn domain riêng cho API Rust, trỏ DNS về VPS. `rust-api.example.com` chỉ là
placeholder; thay tất cả các chỗ trong mẫu và env. Tạo site HTTP ban đầu bằng
`sudo nano /etc/nginx/sites-available/pathnote-rust`:

```nginx
server {
    listen 80;
    server_name rust-api.example.com;
    location / { proxy_pass http://127.0.0.1:3000; }
}
```

```sh
sudo ln -s /etc/nginx/sites-available/pathnote-rust /etc/nginx/sites-enabled/pathnote-rust
sudo nginx -t
sudo systemctl reload nginx
sudo certbot --nginx -d rust-api.example.com
```

Sau khi có certificate, áp các thiết lập từ `nginx.conf.example` vào site HTTPS
(certbot đã tạo), giữ redirect HTTP → HTTPS. Kiểm tra và reload Nginx. Không ghi đè
site Laravel hiện có. Chỉ cần public SSH/HTTP/HTTPS; API port 3000 và PostgreSQL
5432 vẫn ở localhost. Media được phục vụ qua API có kiểm tra quyền.

## 5. GitHub Actions

Trong repo GitHub, tạo Environment **production**, thêm secrets:

| Secret | Giá trị |
| --- | --- |
| SSH_HOST | IP hoặc hostname VPS |
| SSH_USER | deploy |
| SSH_PORT | 22 hoặc cổng SSH thực tế |
| SSH_PRIVATE_KEY | Private key CI, không có passphrase |
| SSH_KNOWN_HOSTS | Host key VPS đã xác minh |

Lấy host public key từ phiên SSH đã tin cậy trên VPS:

```sh
sudo cat /etc/ssh/ssh_host_ed25519_key.pub
sudo ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

SSH_KNOWN_HOSTS có dạng `VPS_HOST ssh-ed25519 PUBLIC_KEY` nếu port 22, hoặc
`[VPS_HOST]:PORT ssh-ed25519 PUBLIC_KEY` nếu port khác. Dùng đúng hostname/IP của
SSH_HOST; đối chiếu fingerprint qua kênh SSH tin cậy. Workflow bắt buộc host key
khớp, không tự tin host key lấy từ mạng.

Thêm variables (repository hoặc Environment production):

| Variable | Giá trị |
| --- | --- |
| HEALTHCHECK_URL | https://DOMAIN_API_RUST/health (bắt buộc) |
| BUILD_RUNNER | ubuntu-24.04 cho x86_64; ubuntu-24.04-arm cho aarch64 |
| DEPLOY_PATH | /opt/pathnote-rust (mặc định) |

BUILD_RUNNER phải đặt ở repository variables vì build không dùng Environment
production. Nếu đổi DEPLOY_PATH, sửa cả service, env và sudoers tương ứng.

Chạy workflow **Rust CI and deployment** từ tab Actions hoặc push master.
Pipeline chạy migration trước, đổi symlink current, restart service rồi kiểm tra
health local và HTTPS, bao gồm commit SHA của process. Giữ ba release gần nhất.
Khi health lỗi sẽ phục hồi code trước đó; **không rollback schema/database**.
Migration mới phải tương thích code cũ. Lần deploy đầu lỗi sẽ dừng service.

## 6. Kiểm tra sau deploy và sao lưu

```sh
sudo systemctl status pathnote-rust.service --no-pager -l
sudo journalctl -u pathnote-rust.service -n 100 --no-pager
curl -fsS http://127.0.0.1:3000/health
curl -fsS https://DOMAIN_API_RUST/health
```

Thử Google login thật từ frontend sau khi frontend trỏ API Rust. Tests dùng Google
giả lập, không thay thế bước này. Chỉ deploy backend không tự deploy frontend hay
Hocuspocus. Các tác vụ nền hiện chạy trong Rust, không cần Redis/Horizon.

Trước các migration thay đổi dữ liệu, lưu backup ngoài releases:

```sh
sudo install -d -m 700 /var/backups/pathnote-rust
sudo sh -c 'umask 077; sudo -u postgres pg_dump -Fc pathnote_rust > /var/backups/pathnote-rust/database.dump'
```

Lệnh ghi đè bản backup cùng tên; dùng tên theo ngày để lưu nhiều bản. Sao lưu cả
shared/media và khóa JWT vào nơi riêng tư ngoài VPS; thử phục hồi trước khi cần.

## 7. pgAdmin trên Mac, kết nối VPS

Cài pgAdmin Desktop trên Mac. Add New Server:

| Tab | Trường | Giá trị |
| --- | --- | --- |
| General | Name | Pathnote VPS |
| Connection | Host / Port | 127.0.0.1 / 5432 |
| Connection | Maintenance database / Username | pathnote_rust / pathnote |
| Connection | Password | Password database đã tạo |
| SSH Tunnel | Use SSH tunneling | Yes |
| SSH Tunnel | Tunnel host / Port | VPS_HOST / cổng SSH |
| SSH Tunnel | Username | User SSH của anh |
| SSH Tunnel | Authentication / Identity file | Identity file / khóa SSH trên Mac |

pgAdmin đã hỗ trợ SSH tunnel sẵn, không cần cài giao diện quản trị trên VPS hoặc
mở PostgreSQL ra Internet. [Tài liệu pgAdmin](https://www.pgadmin.org/docs/pgadmin4/latest/server_dialog.html).
