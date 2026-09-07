#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fission_bin=${FISSION_BIN:-fission}
host=${COIL_SITE_HOST:-127.0.0.1}
port=${COIL_SITE_PORT:-18123}
evidence_dir=${COIL_SITE_EVIDENCE_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/coil-site-serve.XXXXXXXX")}

mkdir -p "$evidence_dir"
server_log="$evidence_dir/server.log"

set -m
"$fission_bin" site serve \
  --project-dir "$repo_root/website" \
  --host "$host" \
  --port "$port" \
  --no-open >"$server_log" 2>&1 &
server_pid=$!
set +m

cleanup() {
  kill -- "-$server_pid" 2>/dev/null || true
  wait "$server_pid" 2>/dev/null || true
}
trap cleanup EXIT

base_url="http://$host:$port"
ready=0
for _attempt in $(seq 1 900); do
  if ! kill -0 "$server_pid" 2>/dev/null; then
    cat "$server_log" >&2
    exit 1
  fi
  if curl -fsS "$base_url/" -o "$evidence_dir/home.html" 2>/dev/null; then
    ready=1
    break
  fi
  sleep 1
done

if [[ "$ready" -ne 1 ]]; then
  cat "$server_log" >&2
  echo "Coil website did not become ready at $base_url" >&2
  exit 1
fi

home_status=$(curl -sS -o "$evidence_dir/home.html" -w '%{http_code}' "$base_url/")
docs_status=$(curl -sS -o "$evidence_dir/docs.html" -w '%{http_code}' "$base_url/docs/intro/")
architecture_status=$(curl -sS -o "$evidence_dir/architecture.html" -w '%{http_code}' "$base_url/architecture/100-fission-native-application-architecture/")
asset_status=$(curl -sS -o "$evidence_dir/favicon.svg" -w '%{http_code}' "$base_url/img/favicon.svg")
missing_status=$(curl -sS -o /dev/null -w '%{http_code}' "$base_url/not-a-real-coil-route")

test "$home_status" = 200
test "$docs_status" = 200
test "$architecture_status" = 200
test "$asset_status" = 200
test "$missing_status" = 404
rg -q 'Build the product' "$evidence_dir/home.html"
rg -q 'Coil' "$evidence_dir/docs.html"
rg -q 'Fission' "$evidence_dir/architecture.html"
rg -q '<svg' "$evidence_dir/favicon.svg"

printf 'home_status=%s\ndocs_status=%s\narchitecture_status=%s\nasset_status=%s\nmissing_status=%s\nevidence_dir=%s\n' \
  "$home_status" \
  "$docs_status" \
  "$architecture_status" \
  "$asset_status" \
  "$missing_status" \
  "$evidence_dir"
