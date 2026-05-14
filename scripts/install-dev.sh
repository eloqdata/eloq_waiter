#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)
ELOQCTL_HOME=${ELOQCTL_HOME:-"${HOME}/.eloqctl"}
BIN_DIR="${ELOQCTL_HOME}/bin"
CONFIG_LINK="${ELOQCTL_HOME}/config"
CONFIG_SOURCE="${REPO_ROOT}/src/cluster_mgr/config"

mkdir -p "${BIN_DIR}"

cargo build -p cluster_mgr --manifest-path "${REPO_ROOT}/Cargo.toml"
cp "${REPO_ROOT}/target/debug/cluster_mgr" "${BIN_DIR}/cluster_mgr"
chmod 755 "${BIN_DIR}/cluster_mgr"

if [ -e "${CONFIG_LINK}" ] && [ ! -L "${CONFIG_LINK}" ]; then
    BACKUP_PATH="${CONFIG_LINK}.backup.$(date +%Y%m%d%H%M%S)"
    mv "${CONFIG_LINK}" "${BACKUP_PATH}"
    echo "Moved existing config to ${BACKUP_PATH}"
fi

rm -f "${CONFIG_LINK}"
ln -s "${CONFIG_SOURCE}" "${CONFIG_LINK}"

echo "Installed dev eloqctl: ${BIN_DIR}/cluster_mgr"
echo "Linked config: ${CONFIG_LINK} -> ${CONFIG_SOURCE}"
