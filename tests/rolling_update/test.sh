#!/bin/bash
# End-to-end test: rolling config apply in Docker containers via SSH.
#
# Prerequisites:
#   1. Docker
#   2. scripts/install-dev.sh
#
# Usage:
#   cd eloq_waiter
#   scripts/install-dev.sh
#   bash tests/rolling_update/test.sh

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
source "${REPO_ROOT}/tests/docker_env.sh"
TOPO="${SCRIPT_DIR}/topology.generated.yaml"
TOPO_V2="${SCRIPT_DIR}/topology_v2.generated.yaml"
CLUSTER="test-rolling-standby"
LAUNCH_TIMEOUT_SECONDS="${LAUNCH_TIMEOUT_SECONDS:-120}"
STATUS_TIMEOUT_SECONDS="${STATUS_TIMEOUT_SECONDS:-120}"

cleanup() {
    rc=$?
    "${ELOQCTL}" stop "${CLUSTER}" --all --force >/dev/null 2>&1 || true
    "${ELOQCTL}" remove "${CLUSTER}" --force >/dev/null 2>&1 || true
    compose_down
    if [ "${KEEP_E2E_LOGS:-0}" != "1" ]; then
        rm -f "${SCRIPT_DIR}/launch.log" "${SCRIPT_DIR}/status.log" "${TOPO}" "${TOPO_V2}"
    fi
    exit "${rc}"
}
trap cleanup EXIT

render_topology "${SCRIPT_DIR}/topology.yaml" "${TOPO}"
render_topology "${SCRIPT_DIR}/topology_v2.yaml" "${TOPO_V2}"
start_docker_env

echo "[1/5] Launch cluster (checkpoint_interval=120)"
"${ELOQCTL}" stop "${CLUSTER}" --all --force >/dev/null 2>&1 || true
"${ELOQCTL}" remove "${CLUSTER}" --force >/dev/null 2>&1 || true
set +e
run_with_progress "${LAUNCH_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/launch.log" "${ELOQCTL}" launch "${TOPO}"
LAUNCH_RC=$?
set -e
[ ${LAUNCH_RC} -ne 0 ] && { echo "FAIL: launch exited ${LAUNCH_RC}"; dump_failure_diagnostics "${SCRIPT_DIR}/launch.log"; exit 1; }
grep -q "FAIL" "${SCRIPT_DIR}/launch.log" && { echo "FAIL in launch:"; dump_failure_diagnostics "${SCRIPT_DIR}/launch.log"; exit 1; }
echo "  OK"

echo "[2/5] Verify cluster status"
run_with_progress "${STATUS_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/status.log" "${ELOQCTL}" status "${CLUSTER}" --wait 60 || { echo "FAIL: status --wait failed"; dump_failure_diagnostics "${SCRIPT_DIR}/status.log"; exit 1; }
echo "  OK"

echo "[3/5] Compare modified YAML (checkpoint_interval=130)"
diff "${TOPO}" "${TOPO_V2}" || true

echo "[4/5] Apply modified topology"
start_ts=$(date +%s)
"${ELOQCTL}" apply "${TOPO_V2}" 2>&1
elapsed=$(($(date +%s) - start_ts))
echo "  apply done (${elapsed}s)"

echo "[5/5] Verify final status"
run_with_progress "${STATUS_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/status.log" "${ELOQCTL}" status "${CLUSTER}" --wait 60 || { echo "FAIL: final status --wait failed"; dump_failure_diagnostics "${SCRIPT_DIR}/status.log"; exit 1; }
echo "  OK"

echo ""
echo "PASS: rolling apply completed, cluster healthy"
