#!/bin/sh
set -eu

APP_ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
CONFIG_DIR="$APP_ROOT/.cargo"
CONFIG_PATH="$CONFIG_DIR/config.toml"

mkdir -p "$CONFIG_DIR"

cat >"$CONFIG_PATH" <<'EOF'
[patch.crates-io]
davenda-admin = { path = "../../crates/davenda-admin" }
davenda-all = { path = "../../crates/davenda-all" }
davenda-app = { path = "../../crates/davenda-app" }
davenda-assets = { path = "../../crates/davenda-assets" }
davenda-auth = { path = "../../crates/davenda-auth" }
davenda-cms = { path = "../../crates/davenda-cms" }
davenda-commerce = { path = "../../crates/davenda-commerce" }
davenda-config = { path = "../../crates/davenda-config" }
davenda-core = { path = "../../crates/davenda-core" }
davenda-customer-sdk = { path = "../../crates/davenda-customer-sdk" }
davenda-data = { path = "../../crates/davenda-data" }
davenda-events = { path = "../../crates/davenda-events" }
davenda-media = { path = "../../crates/davenda-media" }
davenda-memberships = { path = "../../crates/davenda-memberships" }
davenda-ops = { path = "../../crates/davenda-ops" }
davenda-runtime = { path = "../../crates/davenda-runtime" }
EOF

echo "wrote repo-local Cargo override to $CONFIG_PATH"
