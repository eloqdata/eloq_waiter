# Command Idempotency

`eloqctl` commands should be safe to retry after partial failures. A retry must converge toward the requested state rather
than blindly repeating destructive work.

## Principles

1. Read-only commands never mutate local state or remote hosts: `status`, `export`, `list`, `versions`, `connect`, `plan`.
2. Declarative mutation commands compute a plan first: `apply`.
3. Bootstrap is guarded by live tx observation. If a tx service is already running, `install` skips bootstrap.
4. `deploy` and `launch` upsert the local cluster index so a retry does not fail only because the cluster is already known.
5. Remote start/stop commands must check process state before acting.
6. SQLite task success records are only retry hints. Live observed state is the final authority.
7. Destructive cleanup must remain explicit: `remove --force` may clear local state, but remote cleanup failures are reported.
8. Mutation commands acquire a local cluster lock under `$ELOQCTL_HOME/locks` so concurrent operations on the same cluster fail fast.
9. Stale local locks are reclaimed when the recorded `pid` no longer exists on the local host.

## Current Command Semantics

| Command | Idempotency target |
| --- | --- |
| `plan <yaml>` | Always read-only. Re-runs live observation and prints a plan. |
| `apply <yaml>` | Builds one `ReconcilePlan`, gates on live critical service health, executes actions, then re-observes. |
| `launch <yaml>` | Retry-safe for local cluster index; deploy tasks skip recorded successes; start/log/monitor tasks check process state. |
| `deploy <yaml>` | Retry-safe for local cluster index and upload/unpack success records. |
| `install <cluster>` | Skips bootstrap if live tx service is already running. |
| `start <cluster>` | Starts only missing processes where control tasks can identify existing PIDs. |
| `stop <cluster>` | Stops only running processes where control tasks can identify existing PIDs. |
| `log-service start/stop` | Uses log process status before start/stop and probes readiness after start. |
| `monitor start/stop` | Re-uploads monitor config on start and relies on component control tasks for process checks. |
| `status <cluster>` | Always live observation from known cluster metadata; summary parsing shares the same observed-state model used by `plan`/`apply`. |
| `scale <cluster>` | Duplicate add/remove requests are no-ops before task execution. |
| `scalelog <cluster>` | Duplicate log add/remove requests are no-ops before task execution. |
| `update` / `update-conf` | Gated by live critical-service health before execution and re-verified after update. |
| `backup start` | Intentionally creates a new snapshot; gated by live critical-service health. |
| `backup list/remove` | List is read-only; remove with no matching snapshots is a no-op. |
| `remove <cluster>` | Best-effort remote cleanup; `--force` clears local state even if remote cleanup is incomplete. |

## Remaining Hardening Work

The following commands still need deeper action-level reconcile to be fully robust under every failure mode:

1. `scale` and `scalelog`: remaining work should become desired YAML changes plus reconcile actions.
2. `update` and `update-conf`: remaining work should become version/config desired state changes plus reconcile actions.
3. `backup start`: should support an optional user-provided idempotency key if repeat prevention is required.
4. Upload/unpack actions should eventually verify remote file checksum instead of relying on task history.
5. File transfer still uses the system `scp` binary, but upload now executes `scp` with an argv list instead of through a shell string, and copy failures include captured stdout/stderr.

## Local Mutation Locks

`eloqctl` writes one lock file per cluster under `$ELOQCTL_HOME/locks/<cluster>.lock`. The file records `pid`, `cluster`, `command`, and `created_at`.

If another mutation command finds an existing lock for the same cluster, it fails fast unless the recorded process is no longer present under `/proc/<pid>`. In that stale-lock case, `eloqctl` removes the old lock and retries acquisition.

This is a local safety guard only. It prevents accidental concurrent mutations from the same control host, but it is not a distributed lock across multiple operator machines.
