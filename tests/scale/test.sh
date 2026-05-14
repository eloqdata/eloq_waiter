#!/bin/bash
# End-to-end test: `eloqctl scale` in Docker containers via SSH.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
source "${REPO_ROOT}/tests/docker_env.sh"
TOPO="${SCRIPT_DIR}/topology.generated.yaml"
CLUSTER="test-scale-standby"
LAUNCH_TIMEOUT_SECONDS="${LAUNCH_TIMEOUT_SECONDS:-120}"
STATUS_TIMEOUT_SECONDS="${STATUS_TIMEOUT_SECONDS:-120}"

cleanup() {
    rc=$?
    "${ELOQCTL}" stop "${CLUSTER}" --all --force >/dev/null 2>&1 || true
    "${ELOQCTL}" remove "${CLUSTER}" --force >/dev/null 2>&1 || true
    compose_down
    if [ "${KEEP_E2E_LOGS:-0}" != "1" ]; then
        rm -f "${SCRIPT_DIR}/launch.log" "${SCRIPT_DIR}/status.log" "${TOPO}"
    fi
    exit "${rc}"
}
trap cleanup EXIT

render_topology "${SCRIPT_DIR}/topology.yaml" "${TOPO}"
start_docker_env

echo "[1/5] Launch cluster (1 master + 1 standby)"
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

echo "[3/5] Scale add: new replica at 172.28.10.11:6390"
"${ELOQCTL}" scale "${CLUSTER}" \
    --add-nodes 172.28.10.11:6390 \
    --ng-id 0 \
    --is-candidate true 2>&1
run_with_progress "${STATUS_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/status.log" "${ELOQCTL}" status "${CLUSTER}" --wait 60 || { echo "FAIL: status after add failed"; dump_failure_diagnostics "${SCRIPT_DIR}/status.log"; exit 1; }
echo "  status after add: OK"

echo "[4/5] Scale remove: old standby at 172.28.10.12:6379"
"${ELOQCTL}" scale "${CLUSTER}" \
    --remove-nodes 172.28.10.12:6379 2>&1
run_with_progress "${STATUS_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/status.log" "${ELOQCTL}" status "${CLUSTER}" --wait 60 || { echo "FAIL: status after remove failed"; dump_failure_diagnostics "${SCRIPT_DIR}/status.log"; exit 1; }
echo "  status after remove: OK"

echo "[5/5] Final state"
"${ELOQCTL}" status "${CLUSTER}" --wait 60

echo ""
echo "PASS: scale add and remove completed, cluster healthy"
