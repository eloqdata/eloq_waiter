# Upgrades

## Upgrade `eloqctl`

Install the desired release tag with the installer:

```sh
curl -fsSL https://raw.githubusercontent.com/eloqdb/eloq_waiter/main/install.sh | sh -s -- vX.Y.Z
```

For local development builds, reinstall from the current checkout:

```sh
scripts/install-dev.sh
```

## Upgrade Local State Schema

Run the SQLite schema upgrade command after installing a newer `eloqctl` if local state needs migration:

```sh
eloqctl upgrade
```

Current state storage keeps launch-compatible topology YAML under `$ELOQCTL_HOME/clusters/<cluster>/topology.yaml` and stores only a cluster index plus operational metadata in SQLite.

## Upgrade EloqKV Cluster Version

Upgrade an existing EloqKV cluster to a specific version:

```sh
eloqctl update <cluster> <version>
```

Upgrade to the latest available version:

```sh
eloqctl update <cluster> latest
```

Use `--force` only when graceful shutdown is impossible or the cluster is already down:

```sh
eloqctl update <cluster> <version> --force
```

Use `eloqctl status <cluster> --wait 60` after an upgrade to verify live health.
