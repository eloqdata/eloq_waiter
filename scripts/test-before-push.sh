#!/bin/bash

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "[1/6] Install dev eloqctl"
"${REPO_ROOT}/scripts/install-dev.sh"

echo "[2/6] Check formatting"
cargo fmt --all -- --check

echo "[3/6] Check cluster_mgr"
cargo check -p cluster_mgr

echo "[4/6] Run Docker HA E2E"
bash tests/docker_ha/test.sh

echo "[5/6] Run Docker rolling update E2E"
bash tests/rolling_update/test.sh

echo "[6/6] Run Docker scale E2E"
bash tests/scale/test.sh

echo "PASS: pre-push test suite completed"
