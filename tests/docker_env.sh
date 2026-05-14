#!/bin/bash

set -eo pipefail

DOCKER_E2E_DIR="${DOCKER_E2E_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/docker_ha" && pwd)}"
REPO_ROOT="${REPO_ROOT:-$(cd "${DOCKER_E2E_DIR}/../.." && pwd)}"
export ELOQCTL_HOME="${ELOQCTL_HOME:-${HOME}/.eloqctl}"
ELOQCTL="${ELOQCTL:-${ELOQCTL_HOME}/bin/cluster_mgr}"
ELOQCTL_DOCKER_SSH_KEY="${ELOQCTL_DOCKER_SSH_KEY:-${DOCKER_E2E_DIR}/id_ed25519}"
export ELOQCTL_DOCKER_SSH_KEY
CLEANUP_TIMEOUT_SECONDS="${CLEANUP_TIMEOUT_SECONDS:-20}"

compose() {
    if docker compose version >/dev/null 2>&1; then
        docker compose -f "${DOCKER_E2E_DIR}/docker-compose.yaml" "$@"
    else
        docker-compose -f "${DOCKER_E2E_DIR}/docker-compose.yaml" "$@"
    fi
}

compose_down() {
    if docker compose version >/dev/null 2>&1; then
        timeout --kill-after=5s "${CLEANUP_TIMEOUT_SECONDS}s" docker compose -f "${DOCKER_E2E_DIR}/docker-compose.yaml" down -v >/dev/null 2>&1 || true
    else
        timeout --kill-after=5s "${CLEANUP_TIMEOUT_SECONDS}s" docker-compose -f "${DOCKER_E2E_DIR}/docker-compose.yaml" down -v >/dev/null 2>&1 || true
    fi
}

ssh_cmd() {
    ssh -o UserKnownHostsFile=/dev/null \
        -o StrictHostKeyChecking=no \
        -o PasswordAuthentication=no \
        -o BatchMode=yes \
        -o ConnectTimeout=3 \
        -i "${ELOQCTL_DOCKER_SSH_KEY}" \
        eloq@127.0.0.1 \
        -p "$1" \
        "${@:2}"
}

ensure_dev_eloqctl() {
    if [ ! -d "${ELOQCTL_HOME}/config" ] || [ ! -x "${ELOQCTL}" ]; then
        "${REPO_ROOT}/scripts/install-dev.sh"
    fi
}

ensure_ssh_key() {
    if [ ! -f "${ELOQCTL_DOCKER_SSH_KEY}" ]; then
        ssh-keygen -t ed25519 -N '' -f "${ELOQCTL_DOCKER_SSH_KEY}" >/dev/null
    fi
    cp "${ELOQCTL_DOCKER_SSH_KEY}.pub" "${DOCKER_E2E_DIR}/authorized_keys"
}

render_topology() {
    local source_topology="$1"
    local rendered_topology="$2"
    sed "s|\${ELOQCTL_DOCKER_SSH_KEY}|${ELOQCTL_DOCKER_SSH_KEY}|g" "${source_topology}" > "${rendered_topology}"
}

start_docker_env() {
    ensure_dev_eloqctl
    ensure_ssh_key

    compose_down

    echo "[docker] Build Ubuntu SSH containers"
    COMPOSE_PROGRESS=plain compose build

    echo "[docker] Start Docker HA network"
    compose up -d >/dev/null

    echo "[docker] Wait for SSH"
    for host in 2221 2222 2223; do
        for _ in $(seq 1 60); do
            if ssh_cmd "${host}" true >/dev/null 2>&1; then
                break
            fi
            sleep 1
        done
        ssh_cmd "${host}" true >/dev/null || {
            echo "FAIL: SSH is not ready on 127.0.0.1:${host}"
            compose ps || true
            compose logs --no-color --tail=80 || true
            exit 1
        }
    done
}

dump_failure_diagnostics() {
    local log_file="$1"
    echo "---- ${log_file} ----"
    if [ -f "${log_file}" ]; then
        tail -80 "${log_file}" || true
    else
        echo "missing"
    fi
    echo "---- eloqctl command logs ----"
    if [ -d "${ELOQCTL_HOME}/logs" ]; then
        ls -lt "${ELOQCTL_HOME}/logs" || true
        for file in "${ELOQCTL_HOME}"/logs/last-*.log; do
            [ -f "${file}" ] || continue
            echo "---- ${file} ----"
            tail -80 "${file}" || true
        done
    fi
    echo "---- docker status ----"
    compose ps || true
    compose logs --no-color --tail=80 || true
}

run_with_progress() {
    local timeout_seconds="$1"
    local log_file="$2"
    shift 2

    : > "${log_file}"
    timeout --kill-after=10s "${timeout_seconds}s" "$@" > "${log_file}" 2>&1 &
    local cmd_pid=$!
    local elapsed=0
    while kill -0 "${cmd_pid}" >/dev/null 2>&1; do
        sleep 5
        elapsed=$((elapsed + 5))
        echo "  ... still running after ${elapsed}s: $*"
        if [ -s "${log_file}" ]; then
            echo "  ---- recent command output ----"
            tail -20 "${log_file}" || true
        fi
        if [ -f "${ELOQCTL_HOME}/logs/last-launch.log" ]; then
            echo "  ---- recent eloqctl log ----"
            tail -20 "${ELOQCTL_HOME}/logs/last-launch.log" || true
        fi
    done
    wait "${cmd_pid}"
}
