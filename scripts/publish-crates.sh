#!/usr/bin/env bash
set -euo pipefail

PACKAGES=(
  coil-a11y
  coil-assets
  coil-cache
  coil-commerce
  coil-core
  coil-customer-sdk
  coil-data
  coil-i18n
  coil-observability
  coil-report
  coil-seo
  coil-storage
  coil-template
  coil-tls
  coil-wasm
  coil-auth
  coil-config
  coil-app
  coil-admin
  coil-cms
  coil-events
  coil-jobs
  coil-media
  coil-memberships
  coil-ops
  coil-runtime
  coil-import
  coil-cli
  coil
)

MODE="${DRY_RUN:-false}"

for package in "${PACKAGES[@]}"; do
  if [[ "$MODE" == "true" ]]; then
    echo "cargo package -p $package --locked"
    cargo package -p "$package" --locked
  else
    echo "cargo publish -p $package --locked"
    cargo publish -p "$package" --locked
    sleep 30
  fi
done
