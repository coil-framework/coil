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

echo "Starting OctoHub local stack bootstrap"

wait_for_tcp postgres 5432 postgres
wait_for_tcp redis 6379 redis
wait_for_http http://minio:9000/minio/health/live minio

echo "Applying OctoHub executable migrations"
octohub --config "$CONFIG_PATH" migrate apply --yes

echo "Publishing OctoHub theme assets"
octohub --config "$CONFIG_PATH" assets publish

echo "Launching OctoHub dev server"
exec octohub --config "$CONFIG_PATH" up
