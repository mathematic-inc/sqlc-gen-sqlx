#!/usr/bin/env bash
set -euo pipefail

UPSTREAM_REPO="sqlc-dev/sqlc"
UPSTREAM_BRANCH="main"
PROTO_PATH="protos/plugin/codegen.proto"
OUT_DIR="proto/plugin"
LOCK_FILE="proto/UPSTREAM.lock"

SHA=$(curl -sSL \
    -H "Accept: application/vnd.github.v3+json" \
    "https://api.github.com/repos/${UPSTREAM_REPO}/commits?path=${PROTO_PATH}&sha=${UPSTREAM_BRANCH}&per_page=1" \
    | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['sha'])")

mkdir -p "$OUT_DIR"
curl -sSL \
    "https://raw.githubusercontent.com/${UPSTREAM_REPO}/${SHA}/${PROTO_PATH}" \
    -o "${OUT_DIR}/codegen.proto"

echo "${SHA}" > "${LOCK_FILE}"
echo "Synced ${PROTO_PATH} at commit ${SHA}"
