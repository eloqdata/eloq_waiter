#!/bin/bash

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "[1/6] Install dev eloqctl"
"${REPO_ROOT}/scripts/install-dev.sh"

echo "[2/6] Check formatting"
cargo fmt --all -- --check

echo "[3/7] Check cluster_mgr"
cargo check -p cluster_mgr

echo "[4/7] Run clippy (all targets, all features, warnings as errors)"
cargo clippy --all-targets --all-features -- -D warnings

echo "[5/7] Run Docker HA E2E"
bash tests/docker_ha/test.sh

echo "[6/7] Run Docker rolling update E2E"
bash tests/rolling_update/test.sh

echo "[7/7] Run Docker scale E2E"
bash tests/scale/test.sh

echo "PASS: pre-push test suite completed"
