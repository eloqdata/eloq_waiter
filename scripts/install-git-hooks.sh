#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)

if ! git -C "${REPO_ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "Not inside a git repository: ${REPO_ROOT}" >&2
    exit 1
fi

mkdir -p "${REPO_ROOT}/.git/hooks"
cp "${REPO_ROOT}/.githooks/pre-push" "${REPO_ROOT}/.git/hooks/pre-push"
chmod +x "${REPO_ROOT}/.git/hooks/pre-push"

echo "Installed pre-push hook: ${REPO_ROOT}/.git/hooks/pre-push"
