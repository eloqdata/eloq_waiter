# Developer Helper Commands

`devtools` is the legacy developer helper binary in this workspace. New deployment and operations work should use `eloqctl`.

## Scope

`devtools` is intended for local source checkout workflows such as dependency setup, workspace layout, and developer build helpers. It is not the production cluster-management interface.

For cluster operations, use:

```sh
eloqctl --help
```

## Current Local Development Commands

The most commonly used repository-level commands are:

```sh
cargo check -p cluster_mgr --bin eloqctl
cargo build -p cluster_mgr --bin eloqctl
cargo build -p cluster_mgr --bin eloqctl --release
scripts/install-dev.sh
scripts/test-before-push.sh
scripts/install-git-hooks.sh
```

## Notes

1. `eloqctl` currently targets EloqKV only.
2. Docker E2E tests use the locally installed `eloqctl` and SSH into Ubuntu containers.
3. Legacy MySQL/MariaDB-oriented examples in older devtools documentation no longer describe the active cluster-manager path.
