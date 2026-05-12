#!/bin/bash
# Host-based test: zero-downtime rolling config restart via RollingUpgrade.
#
# Prerequisites: SSH key at ~/.ssh/id_rsa, sshd on localhost
# Usage: cargo build -p cluster_mgr && bash tests/test_rolling_upgrade_host.sh

set -eo pipefail
ELOQCTL="${PWD}/target/debug/cluster_mgr"
TOPO="${PWD}/tests/rolling_upgrade_standby_localhost.yaml"
CLUSTER="test-rolling-standby"
export ELOQCTL_HOME="${HOME}/.eloqctl"
mkdir -p "${ELOQCTL_HOME}"

cleanup() {
    "${ELOQCTL}" stop "${CLUSTER}" --all 2>/dev/null || true
    "${ELOQCTL}" remove "${CLUSTER}" --force 2>/dev/null || true
    kill "$WRITER_PID" 2>/dev/null || true
    rm -f "${WRITE_LOG}" "${ERROR_LOG}" /tmp/rolling_launch.log
}
trap cleanup EXIT

echo "[1/5] Launch"
rm -rf "${HOME}/${CLUSTER}" "${ELOQCTL_HOME}/db/cluster_mgr_state.db" 2>/dev/null || true
set +e
"${ELOQCTL}" launch "${TOPO}" -s > /tmp/rolling_launch.log 2>&1
LAUNCH_RC=$?
set -e
[ ${LAUNCH_RC} -ne 0 ] && { echo "FAIL: launch exited ${LAUNCH_RC}"; tail -20 /tmp/rolling_launch.log; exit 1; }
grep -q "FAIL" /tmp/rolling_launch.log && { echo "FAIL in launch:"; grep FAIL /tmp/rolling_launch.log; exit 1; }
echo "  OK"

echo "[2/5] Wait ready"
CLIENT="$("${ELOQCTL}" -q connect "${CLUSTER}")"
for i in $(seq 1 60); do
    "${CLIENT}" set _t v >/dev/null 2>&1 && { echo "  ready (${i}s)"; break; }
    [ $i -ge 60 ] && { echo "FAIL: not ready after 60s"; exit 1; }
    sleep 1
done
"${CLIENT}" cluster slots

echo "[3/5] Start writes + rolling restart"
WRITE_LOG=$(mktemp); ERROR_LOG=$(mktemp)
(while true; do
    SEQ=$((SEQ+1))
    OUT=$("${CLIENT}" set rolling_k "${SEQ}" 2>&1) || echo "FAIL ${SEQ}" >> "${ERROR_LOG}"
    echo "${SEQ}" >> "${WRITE_LOG}"
    sleep 0.05
done) & WRITER_PID=$!
sleep 2

echo "Note: stop after failover may take ~5min due to graceful_quit_on_sigterm=true"
start_ts=$(date +%s)
"${ELOQCTL}" update-conf "${CLUSTER}" --restart --fields checkpoint_interval:130
elapsed=$(($(date +%s) - start_ts))
echo "  restart done (${elapsed}s)"

sleep 2; kill "$WRITER_PID" 2>/dev/null || true; wait "$WRITER_PID" 2>/dev/null || true

echo "[4/5] Results"
echo "  writes=$(wc -l < "${WRITE_LOG}" 2>/dev/null) errors=$(wc -l < "${ERROR_LOG}" 2>/dev/null)"
[ "$(wc -l < "${ERROR_LOG}" 2>/dev/null || echo 0)" -gt 0 ] && { echo "FAIL: write errors"; head -10 "${ERROR_LOG}"; exit 1; }

echo "[5/5] Verify"
"${CLIENT}" set final_k ok >/dev/null 2>&1
VAL=$("${CLIENT}" get rolling_k 2>/dev/null || echo "N/A")
echo "  last rolling key = ${VAL}"

echo "PASS"
