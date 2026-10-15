#!/usr/bin/env bash
set -euo pipefail

DEPLOY_PATH=${1:?Deploy path required}
RELEASE=${2:?Release required}
HEALTHCHECK_URL=${3:?Public health URL required}
COMMIT_SHA=${4:?Commit SHA required}
SERVICE=pathnote-rust.service
APP_USER=pathnote

[[ "$DEPLOY_PATH" =~ ^/opt/[a-zA-Z0-9_-]+$ ]]
[[ "$RELEASE" =~ ^[0-9]{14}-[0-9]+-[0-9]+$ ]]
[[ "$HEALTHCHECK_URL" =~ ^https://[a-zA-Z0-9.-]+(:[0-9]+)?/health$ ]]
[[ "$COMMIT_SHA" =~ ^[a-f0-9]{40}$ ]]
SHARED="$DEPLOY_PATH/shared"
RELEASE_PATH="$DEPLOY_PATH/releases/$RELEASE"
test -d "$RELEASE_PATH" && test ! -L "$RELEASE_PATH"
test -f "$SHARED/.env"
test -f "$SHARED/keys/access-token.pem"
test -d "$SHARED/media"
test ! -e "$DEPLOY_PATH/current" || test -L "$DEPLOY_PATH/current"
command -v python3 >/dev/null
command -v flock >/dev/null
getent passwd "$APP_USER" >/dev/null
systemctl cat "$SERVICE" >/dev/null

exec 9> "$DEPLOY_PATH/.deploy.lock"
flock -w 300 9
PREVIOUS_RELEASE=''
if [[ -L "$DEPLOY_PATH/current" ]]; then
  PREVIOUS_RELEASE=$(readlink -f "$DEPLOY_PATH/current" || true)
fi
SWITCHED=false

switch_release() {
  ln -sfn "$1" "$DEPLOY_PATH/.current-$RELEASE"
  mv -Tf "$DEPLOY_PATH/.current-$RELEASE" "$DEPLOY_PATH/current"
}
on_exit() {
  local status=$?
  trap - EXIT HUP INT TERM
  if (( status != 0 )) && [[ "$SWITCHED" == true ]]; then
    if [[ -n "$PREVIOUS_RELEASE" && -d "$PREVIOUS_RELEASE" ]]; then
      switch_release "$PREVIOUS_RELEASE"
      sudo -n systemctl restart "$SERVICE" || echo 'Previous Rust service failed to restart.' >&2
      echo 'Deployment failed; previous code release restored.' >&2
    else
      sudo -n systemctl stop "$SERVICE" || true
      rm -f "$DEPLOY_PATH/current"
      echo 'First deployment failed; failed service stopped.' >&2
    fi
  fi
  if (( status != 0 )); then
    echo 'Database migrations are retained; use backward-compatible schema changes.' >&2
  fi
  exit "$status"
}
trap on_exit EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

cd "$RELEASE_PATH"
sha256sum --check --strict SHA256SUMS
test "$(cat deploy-version.txt)" = "$COMMIT_SHA"
case "$(uname -m):$(cat build-target.txt)" in
  x86_64:x86_64-unknown-linux-musl|aarch64:aarch64-unknown-linux-musl) ;;
  *) echo 'Release architecture does not match this VPS.' >&2; exit 1 ;;
esac
chgrp -R "$APP_USER" "$RELEASE_PATH"
chmod 750 "$RELEASE_PATH" server migration
chmod 640 healthcheck.py SHA256SUMS deploy-version.txt build-target.txt
ln -s "$SHARED/.env" .env
# No shell sourcing of .env: the SeaORM CLI reads dotenv itself.
sudo -n -u "$APP_USER" test -r "$SHARED/.env"
sudo -n -u "$APP_USER" test -r "$SHARED/keys/access-token.pem"
sudo -n -u "$APP_USER" test -w "$SHARED/media"
sudo -n -u "$APP_USER" "$RELEASE_PATH/migration" up

SWITCHED=true
switch_release "$RELEASE_PATH"
sudo -n systemctl restart "$SERVICE"

check_release() {
  local url=$1
  for attempt in {1..10}; do
    if python3 "$RELEASE_PATH/healthcheck.py" "$url" "$COMMIT_SHA"; then return 0; fi
    sleep 2
  done
  return 1
}
# Service template fixes APP_BIND to loopback port 3000 in production .env.
echo 'Checking Rust health on localhost...'
check_release http://127.0.0.1:3000/health
echo 'Checking Rust health through public HTTPS...'
check_release "$HEALTHCHECK_URL?release=$RELEASE"
systemctl is-active --quiet "$SERVICE"

# Keep the current, previous and one extra release. No shared-data deletion.
mapfile -t RELEASES < <(find "$DEPLOY_PATH/releases" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort -r)
KEPT=1
if [[ -n "$PREVIOUS_RELEASE" && "$PREVIOUS_RELEASE" != "$RELEASE_PATH" ]]; then KEPT=2; fi
for candidate in "${RELEASES[@]}"; do
  [[ "$candidate" =~ ^[0-9]{14}-[0-9]+-[0-9]+$ ]] || continue
  candidate_path="$DEPLOY_PATH/releases/$candidate"
  [[ "$candidate_path" != "$RELEASE_PATH" && "$candidate_path" != "$PREVIOUS_RELEASE" ]] || continue
  if (( KEPT < 3 )); then KEPT=$((KEPT + 1)); else rm -rf -- "$candidate_path" || echo "Could not prune old release: $candidate" >&2; fi
done
echo "Rust release is healthy: $RELEASE"
