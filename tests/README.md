# E2E Tests

These scripts run real `eloqctl` operations against Ubuntu Docker containers using the locally installed development build. The containers provide only SSH-accessible Ubuntu nodes; `eloqctl` installs runtime dependencies and deploys EloqKV from the host.

## Prerequisites

1. Docker is available.
2. Install the current repo build first:

```sh
scripts/install-dev.sh
```

This builds `cluster_mgr`, installs it to `${ELOQCTL_HOME:-$HOME/.eloqctl}/bin/cluster_mgr`, and links `${ELOQCTL_HOME:-$HOME/.eloqctl}/config` to this repo's `src/cluster_mgr/config`.

## Tests

```sh
bash tests/docker_ha/test.sh
bash tests/rolling_update/test.sh
bash tests/scale/test.sh
```

Each test has its own directory containing the script, topology, and transient logs. The scripts use `${ELOQCTL_HOME:-$HOME/.eloqctl}/bin/cluster_mgr` by default; set `ELOQCTL=/path/to/cluster_mgr` to override. Readiness and cleanup are verified through `eloqctl` itself; every test cleanup runs `eloqctl stop <cluster> --all --force` and `eloqctl remove <cluster> --force`.

Run the full push gate locally with:

```sh
scripts/test-before-push.sh
```

Install the git pre-push hook with:

```sh
scripts/install-git-hooks.sh
```

`tests/docker_ha/test.sh` starts three Ubuntu containers on a Docker bridge network and deploys EloqKV through SSH from the host, which is closer to a real multi-host deployment than localhost tests.

All E2E tests use `connection.ssh_endpoints` for SSH access and `connection.service_endpoints` for Redis/gRPC readiness from the host. The HA baseline is one tx/master, one standby, and one voter with `cluster_mode: true`.
