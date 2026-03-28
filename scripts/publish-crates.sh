#!/usr/bin/env bash
set -euo pipefail

MODE="${DRY_RUN:-false}"
VERSION="${COIL_RELEASE_VERSION:-0.1.0}"
WAIT_TIMEOUT_SECONDS="${WAIT_TIMEOUT_SECONDS:-300}"
WAIT_INTERVAL_SECONDS="${WAIT_INTERVAL_SECONDS:-5}"

mapfile -t PACKAGES < <(python3 - <<'PY'
from pathlib import Path
import tomllib

root = Path.cwd() / "crates"
crates = {}
for manifest in root.glob("*/Cargo.toml"):
    data = tomllib.loads(manifest.read_text())
    name = data["package"]["name"]
    deps = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for dep, spec in data.get(section, {}).items():
            if isinstance(spec, dict) and "path" in spec and dep.startswith("coil"):
                deps.add(dep)
    crates[name] = deps

remaining = {name: deps & crates.keys() for name, deps in crates.items()}
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

for name in order:
    print(name)
PY
)

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
                sys.exit(0)
    except urllib.error.HTTPError as exc:
        if exc.code != 404:
            raise
    except urllib.error.URLError:
        pass
    time.sleep(interval_s)
raise SystemExit(f"timed out waiting for {package} {version} to appear on crates.io")
PY
}

if [[ "$MODE" == "true" ]]; then
  printf 'publish order for version %s:\n' "$VERSION"
  printf '  %s\n' "${PACKAGES[@]}"
  exit 0
fi

for package in "${PACKAGES[@]}"; do
  echo "cargo publish -p $package --locked"
  cargo publish -p "$package" --locked
  wait_for_index "$package" "$VERSION"
done
