#!/usr/bin/env bash
set -euo pipefail

PACKAGES=(
  davenda-a11y
  davenda-assets
  davenda-cache
  davenda-commerce
  davenda-core
  davenda-customer-sdk
  davenda-data
  davenda-i18n
  davenda-observability
  davenda-report
  davenda-seo
  davenda-storage
  davenda-template
  davenda-tls
  davenda-wasm
  davenda-auth
  davenda-config
  davenda-app
  davenda-admin
  davenda-cms
  davenda-events
  davenda-jobs
  davenda-media
  davenda-memberships
  davenda-ops
  davenda-runtime
  davenda-import
  davenda-cli
  davenda-all
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
