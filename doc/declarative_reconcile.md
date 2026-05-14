# Declarative Reconcile Design

## Goals

`eloqctl` should move from command-by-command orchestration toward a declarative workflow:

```shell
eloqctl plan cluster.yaml
eloqctl apply cluster.yaml
eloqctl status cluster-name
```

The YAML file is required only when the user wants to create or change the desired cluster shape.
Operational inspection commands such as `status`, `connect`, and `list` should continue to work from a cluster name.

## Source Of Truth

There are two authoritative sources:

1. Desired state: the user-owned YAML file, usually stored in the filesystem or Git.
2. Observed state: live state collected from hosts, Redis/EloqKV, log service, DSS, and monitor services.

SQLite is not a source of truth for desired or observed state. It should not be used to decide whether a process is running,
whether a port is listening, or whether a cluster is healthy.

## SQLite Role

SQLite remains useful only as local operational metadata:

1. Cluster index for commands that do not take YAML, such as `eloqctl status <cluster>`.
2. Apply lock to prevent concurrent mutation of the same cluster.
3. Operation history and step reports for troubleshooting.
4. Compatibility with existing commands while the declarative path continues to expand.

SQLite does not store the user's YAML as the canonical desired state. `eloqctl` writes the launch-compatible topology file to `$ELOQCTL_HOME/clusters/<cluster>/topology.yaml` and stores only a cluster index entry in SQLite.

## Command Semantics

### `status <cluster>`

`status` does not need YAML. It loads enough cluster metadata from the local index to know which hosts and ports to observe,
then probes the real cluster:

1. SSH process status for tx, standby, voter, log, DSS, and monitor services.
2. Redis/EloqKV ping and cluster topology where applicable.
3. Log service health endpoint and raft leader readiness.
4. Monitor process status.

If the local index is missing, re-register the cluster by running `launch`, `deploy`, or a future explicit import command from a topology file.

### `plan <yaml>`

`plan` is read-only. It should:

1. Load and validate desired YAML.
2. Resolve derived fields needed for planning, such as release artifact URLs.
3. Load cluster index if it exists.
4. Collect live observed state.
5. Produce an action plan.
6. Exit non-zero if the desired state is invalid or live observation is impossible.

The current implementation builds a `ReconcilePlan` from desired topology plus live observation and reports unsupported changes instead of applying them silently.

### `apply <yaml>`

`apply` should run the same planning logic as `plan`, then execute the generated actions. Each action must be idempotent.

Recommended flow:

1. Acquire apply lock for the cluster.
2. Build plan from desired YAML and live observed state.
3. Print the plan.
4. Execute actions in dependency order.
5. Re-observe after important actions.
6. Verify final conditions.
7. Write operation history and update the local cluster index.

## Reconcile Model

Internal model:

```text
DesiredCluster   <- parsed from YAML
ObservedCluster  <- collected live every plan/apply/status run
ClusterIndex     <- minimal local metadata for cluster-name commands
ActionPlan       <- desired + observed diff
Action           <- small, idempotent operation
OperationReport  <- execution result for troubleshooting
```

Current action types include:

1. EnsureRuntimeDeps
2. EnsurePackagePresent
3. EnsureConfigRendered
4. EnsureConfigUploaded
5. EnsureLogServiceStarted
6. EnsureLogServiceReady
7. EnsureBootstrapped
8. EnsureTxStarted
9. EnsureRedisHealthy
10. EnsureMonitorStarted
11. RestartTxWithUpdatedConfig
12. RollingRestart
13. EnsureClusterIndexUpdated

## Migration Status

### Completed

1. `eloqctl plan <yaml>` is read-only.
2. `eloqctl apply <yaml>` prints the same plan before executing.
3. `status` uses live observed state through the same observed-state model used by `plan`/`apply`.
4. Cluster topology YAML is saved in the filesystem; SQLite stores only the cluster index and operational metadata.
5. Mutation commands use a local stale-lock-aware cluster lock.

### Remaining

1. Convert `scale`, `scalelog`, `update`, and `update-conf` into finer-grained reconcile actions.
2. Replace remaining system `scp` transfer paths with a unified Rust remote file-transfer abstraction.
3. Reduce remaining topology cache tables to either rebuildable caches or remove them.
