#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAIT_TIMEOUT_SECONDS="${WAIT_TIMEOUT_SECONDS:-300}"
WAIT_INTERVAL_SECONDS="${WAIT_INTERVAL_SECONDS:-5}"

mapfile -t PUBLISH_PLAN < <(python3 - <<'PY' "$ROOT_DIR"
from pathlib import Path
import tomllib
import sys

root = Path(sys.argv[1])
crates_root = root / "crates"
special_tail = ["coil-scaffold", "cargo-coil"]

crates = {}
for manifest in crates_root.glob("*/Cargo.toml"):
    data = tomllib.loads(manifest.read_text())
    package = data["package"]["name"]
    version = data["package"]["version"]
    deps = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for dep, spec in data.get(section, {}).items():
            dep_package = dep
            if isinstance(spec, dict):
                dep_package = spec.get("package", dep)
            if isinstance(spec, dict) and "path" in spec and (dep_package.startswith("coil-") or dep_package == "coil-rs" or dep_package == "cargo-coil"):
                deps.add(dep_package)
    crates[package] = {"version": version, "deps": deps}

remaining = {name: meta["deps"] & crates.keys() for name, meta in crates.items() if name not in special_tail}
order = []
while remaining:
    ready = sorted(name for name, deps in remaining.items() if not deps)
    if not ready:
        raise SystemExit(f"cycle detected in publish graph: {remaining}")
    order.extend(ready)
    for name in ready:
        remaining.pop(name)
    for deps in remaining.values():
        deps.difference_update(ready)

for name in special_tail:
    if name in crates:
        order.append(name)

for name in order:
    print(f"{name} {crates[name]['version']}")
PY
)

crate_version_exists() {
  local package="$1"
  local version="$2"
  python3 - "$package" "$version" <<'PY'
import sys
import urllib.error
import urllib.request

package, version = sys.argv[1], sys.argv[2]
url = f"https://crates.io/api/v1/crates/{package}/{version}"
try:
    with urllib.request.urlopen(url, timeout=15) as response:
        raise SystemExit(0 if response.status == 200 else 1)
except urllib.error.HTTPError as exc:
    raise SystemExit(1 if exc.code == 404 else 2)
except urllib.error.URLError:
    raise SystemExit(3)
PY
}

wait_for_index() {
  local package="$1"
  local version="$2"
  python3 - "$package" "$version" "$WAIT_TIMEOUT_SECONDS" "$WAIT_INTERVAL_SECONDS" <<'PY'
import sys
import time
import urllib.error
import urllib.request

package, version, timeout_s, interval_s = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
url = f"https://crates.io/api/v1/crates/{package}/{version}"
deadline = time.time() + timeout_s
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=10) as response:
            if response.status == 200:
                raise SystemExit(0)
    except urllib.error.HTTPError as exc:
        if exc.code != 404:
            raise
    except urllib.error.URLError:
        pass
    time.sleep(interval_s)
raise SystemExit(f"timed out waiting for {package} {version} to appear on crates.io")
PY
}

retry_delay_from_log() {
  local logfile="$1"
  python3 - "$logfile" <<'PY'
from datetime import datetime, timezone
from pathlib import Path
import re
import sys

text = Path(sys.argv[1]).read_text()
match = re.search(r"Please try again after (.+?) and see", text)
if not match:
    print(60)
    raise SystemExit(0)

try:
    retry_at = datetime.strptime(match.group(1).strip(), "%a, %d %b %Y %H:%M:%S GMT").replace(tzinfo=timezone.utc)
    seconds = int((retry_at - datetime.now(timezone.utc)).total_seconds()) + 2
    print(max(seconds, 5))
except Exception:
    print(60)
PY
}

if [[ "${DRY_RUN:-false}" == "true" ]]; then
  printf 'publish plan:\n'
  for entry in "${PUBLISH_PLAN[@]}"; do
    printf '  %s\n' "$entry"
  done
  exit 0
fi

cd "$ROOT_DIR"

for entry in "${PUBLISH_PLAN[@]}"; do
  package="${entry%% *}"
  version="${entry##* }"

  if crate_version_exists "$package" "$version"; then
    echo "Skipping $package $version (already published)"
    continue
  fi

  while true; do
    echo "=== Publishing $package $version at $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    logfile="$(mktemp)"

    if cargo publish -p "$package" --locked --allow-dirty >"$logfile" 2>&1; then
      cat "$logfile"
      rm -f "$logfile"
      wait_for_index "$package" "$version"
      sleep 1
      break
    fi

    cat "$logfile"

    if rg -q "Too Many Requests|status 429|Please try again after" "$logfile"; then
      delay="$(retry_delay_from_log "$logfile")"
      rm -f "$logfile"
      echo "429 while publishing $package $version; sleeping ${delay}s before retry"
      sleep "$delay"
      continue
    fi

    rm -f "$logfile"
    exit 1
  done
done
