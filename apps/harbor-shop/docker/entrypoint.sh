#!/bin/sh
set -eu

CONFIG_PATH="${DAVENDA_CONFIG:-platform.dev.toml}"

wait_for_tcp() {
  host="$1"
  port="$2"
  name="$3"

  until nc -z "$host" "$port" >/dev/null 2>&1; do
    echo "waiting for ${name} on ${host}:${port}"
    sleep 1
  done
}

wait_for_http() {
  url="$1"
  name="$2"

  until curl -fsS "$url" >/dev/null 2>&1; do
    echo "waiting for ${name} at ${url}"
    sleep 1
  done
}

echo "Starting Harbor Shop local stack bootstrap"

wait_for_tcp postgres 5432 postgres
wait_for_tcp redis 6379 redis
wait_for_http http://minio:9000/minio/health/live minio

echo "Validating Harbor Shop config"
platform config validate --config "$CONFIG_PATH"

echo "Applying Harbor Shop migrations"
platform migrate apply --config "$CONFIG_PATH" --yes

echo "Publishing Harbor Shop assets"
platform assets publish --config "$CONFIG_PATH" --yes

echo "Launching Harbor Shop dev server"
if [ "${STRIPE_SECRET_KEY:-sk_test_replace_me}" = "sk_test_replace_me" ]; then
  echo "info: STRIPE_SECRET_KEY is still the placeholder value; Harbor Shop will use the built-in local checkout stub until you override it with a real Stripe test key"
fi
exec platform dev server --config "$CONFIG_PATH"
