#!/bin/sh
set -eu

APP_ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
CONFIG_DIR="$APP_ROOT/.cargo"
CONFIG_PATH="$CONFIG_DIR/config.toml"

mkdir -p "$CONFIG_DIR"

cat >"$CONFIG_PATH" <<'EOF'
[patch.crates-io]
coil-admin = { path = "../../crates/coil-admin" }
coil = { package = "coil-rs", path = "../../crates/coil" }
coil-app = { path = "../../crates/coil-app" }
coil-assets = { path = "../../crates/coil-assets" }
coil-auth = { path = "../../crates/coil-auth" }
coil-cms = { path = "../../crates/coil-cms" }
coil-commerce = { path = "../../crates/coil-commerce" }
coil-config = { path = "../../crates/coil-config" }
coil-core = { path = "../../crates/coil-core" }
coil-customer-sdk = { path = "../../crates/coil-customer-sdk" }
coil-data = { path = "../../crates/coil-data" }
coil-events = { path = "../../crates/coil-events" }
coil-media = { path = "../../crates/coil-media" }
coil-memberships = { path = "../../crates/coil-memberships" }
coil-ops = { path = "../../crates/coil-ops" }
coil-runtime = { path = "../../crates/coil-runtime" }
coil-wasm = { path = "../../crates/coil-wasm" }
EOF

echo "wrote repo-local Cargo override to $CONFIG_PATH"
