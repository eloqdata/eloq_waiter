#!/bin/bash
# End-to-end test: deploy EloqKV HA into Ubuntu Docker containers via SSH.

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DOCKER_E2E_DIR="${SCRIPT_DIR}"
source "${REPO_ROOT}/tests/docker_env.sh"
CLUSTER="test-docker-ha"
TOPO="${SCRIPT_DIR}/topology.generated.yaml"
LAUNCH_TIMEOUT_SECONDS="${LAUNCH_TIMEOUT_SECONDS:-120}"
STATUS_TIMEOUT_SECONDS="${STATUS_TIMEOUT_SECONDS:-120}"

cleanup() {
    rc=$?
    timeout --kill-after=5s "${CLEANUP_TIMEOUT_SECONDS}s" "${ELOQCTL}" stop "${CLUSTER}" --all --force >/dev/null 2>&1 || true
    timeout --kill-after=5s "${CLEANUP_TIMEOUT_SECONDS}s" "${ELOQCTL}" remove "${CLUSTER}" --force >/dev/null 2>&1 || true
    compose_down
    if [ "${KEEP_E2E_LOGS:-0}" != "1" ]; then
        rm -f "${SCRIPT_DIR}/launch.log" "${SCRIPT_DIR}/status.log" "${TOPO}"
    fi
    exit "${rc}"
}
trap cleanup EXIT

render_topology "${SCRIPT_DIR}/topology.yaml" "${TOPO}"

start_docker_env

echo "[4/5] Launch EloqKV HA cluster"
"${ELOQCTL}" stop "${CLUSTER}" --all --force >/dev/null 2>&1 || true
"${ELOQCTL}" remove "${CLUSTER}" --force >/dev/null 2>&1 || true
set +e
run_with_progress "${LAUNCH_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/launch.log" "${ELOQCTL}" launch "${TOPO}"
launch_rc=$?
set -e
if [ ${launch_rc} -ne 0 ]; then
    echo "FAIL: launch exited ${launch_rc}"
    dump_failure_diagnostics "${SCRIPT_DIR}/launch.log"
    exit 1
fi
if grep -q "FAIL" "${SCRIPT_DIR}/launch.log"; then
    echo "FAIL in launch:"
    dump_failure_diagnostics "${SCRIPT_DIR}/launch.log"
    exit 1
fi

echo "[5/5] Verify cluster status"
run_with_progress "${STATUS_TIMEOUT_SECONDS}" "${SCRIPT_DIR}/status.log" "${ELOQCTL}" status "${CLUSTER}" --wait 90 || {
    echo "FAIL: status --wait failed"
    dump_failure_diagnostics "${SCRIPT_DIR}/status.log"
    exit 1
}

echo ""
echo "PASS: Docker HA EloqKV cluster deployed and healthy"
