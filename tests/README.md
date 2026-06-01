# E2E Tests

This test suite is documented in two common workflows.

## Workflow 1: run the full test flow with the CLI

Build the local `eloqctl` first:

```sh
cd /home/starrysky/workspace/eloqdata-kernel/eloq_waiter
cargo build -p cluster_mgr
```

Main entrypoint:

```sh
tests/e2e/devctl.sh --help
```

Most commonly used commands:

```sh
# Start the Docker environment
tests/e2e/devctl.sh env-up

# Build + start environment + install into control node + render topology + launch
tests/e2e/devctl.sh full

# Show cluster and monitor status
tests/e2e/devctl.sh status

# Upgrade Grafana only
tests/e2e/devctl.sh grafana-update

# Run the full stress test suite
tests/e2e/devctl.sh stress

# Run only SDK stress tests; traffic stays inside containers, not on the host
tests/e2e/devctl.sh stress py-stress,go-stress,ts-stress

# Run only the RESP compatibility tests (EloqKV vs Redis 7.0 by default)
tests/e2e/devctl.sh stress resp-compat

# Override the Redis target version used by the compatibility suite
RESP_COMPAT_VERSION=6.2.0 tests/e2e/devctl.sh stress resp-compat

# Remove the Docker environment
tests/e2e/devctl.sh env-down
```

Notes:

- `devctl.sh stress` calls `tests/e2e/cmd_stress_test.sh`.
- GitHub Actions uses the same E2E step list and includes `resp-compat` by default.
- Python stress runs in `stress-python`.
- Go stress runs in `stress-go`.
- TypeScript stress runs in `stress-ts`.
- RESP compatibility runs in `resp-compat`.
- `env-up` reuses existing local images by default. To force a rebuild, run `FORCE_DOCKER_BUILD=1 tests/e2e/devctl.sh env-up`.
- `env-up` only creates a fresh Docker test environment. The `eloqctl` runtime directory inside the control node is also reset to a clean state.

## Workflow 2: start the environment, then run `eloqctl` manually inside the control node

### 1. Start the environment

```sh
cd /home/starrysky/workspace/eloqdata-kernel/eloq_waiter
cargo build -p cluster_mgr
tests/e2e/devctl.sh env-up
tests/e2e/devctl.sh install-control
tests/e2e/devctl.sh render-topology
```

### 2. Log in to the control node

```sh
tests/e2e/devctl.sh control-shell
```

Equivalent command:

```sh
ssh -i tests/docker_ha/id_ed25519 -p 2224 eloq@127.0.0.1
```

Important paths inside the control node:

- Repository: `/workspace/eloq_waiter`
- `eloqctl`: `/usr/local/bin/eloqctl`
- `ELOQCTL_HOME`: `/home/eloq/.eloqctl`
- Rendered topology: `/home/eloq/topology.generated.yaml`

### 3. Launch, update, and inspect the cluster manually

Run inside the control node:

```sh
eloqctl stop test-e2e --all --force || true
eloqctl remove test-e2e --force || true
eloqctl launch --skip-deps /home/eloq/topology.generated.yaml
```

Check status:

```sh
eloqctl status test-e2e --wait 180
eloqctl monitor status --cluster test-e2e
```

Upgrade Grafana manually:

```sh
eloqctl monitor update --cluster test-e2e \
  --component grafana \
  --url 'https://dl.grafana.com/grafana/release/13.0.1+security-01/grafana_13.0.1+security-01_25720641773_linux_amd64.tar.gz'
```

Install Alertmanager on an existing cluster:

```sh
eloqctl monitor update --cluster test-e2e \
  --component alertmanager \
  --url 'https://github.com/prometheus/alertmanager/releases/download/v0.32.1/alertmanager-0.32.1.linux-amd64.tar.gz'
```

Re-run the same Alertmanager update:

```sh
eloqctl monitor update --cluster test-e2e \
  --component alertmanager \
  --url 'https://github.com/prometheus/alertmanager/releases/download/v0.32.1/alertmanager-0.32.1.linux-amd64.tar.gz'
```

Install Alertmanager together with `alertmanager-webhook-adapter`:

```sh
eloqctl monitor update --cluster test-e2e \
  --component alertmanager \
  --url 'https://github.com/prometheus/alertmanager/releases/download/v0.32.1/alertmanager-0.32.1.linux-amd64.tar.gz'
```

Install Alertmanager and enable Feishu forwarding at the same time:

```sh
eloqctl monitor update --cluster test-e2e \
  --component alertmanager \
  --url 'https://github.com/prometheus/alertmanager/releases/download/v0.32.1/alertmanager-0.32.1.linux-amd64.tar.gz' \
  --feishu-robot-url 'https://open.feishu.cn/open-apis/bot/v2/hook/xxx'
```

This also deploys `alertmanager-webhook-adapter` and ships the built-in Chinese Feishu template:

- Template language: `zh`
- Default signature: `EloqKV`
- Template file: `src/cluster_mgr/config/feishu.zh.tmpl`
- Remote deployment path: `/home/eloq/test-e2e/alertmanager-webhook-adapter/templates/feishu.zh.tmpl`

Re-run the same command to update Alertmanager again or recover from a failed installation:

```sh
eloqctl monitor update --cluster test-e2e \
  --component alertmanager \
  --url 'https://github.com/prometheus/alertmanager/releases/download/v0.32.1/alertmanager-0.32.1.linux-amd64.tar.gz' \
  --feishu-robot-url 'https://open.feishu.cn/open-apis/bot/v2/hook/xxx'
```

Check monitor status again after installation:

```sh
eloqctl monitor status --cluster test-e2e
```

If you previously deployed the legacy standalone `PrometheusAlert`, clean up leftover processes and directories from the control node with:

```sh
ssh -i /home/eloq/.ssh/id_ed25519 eloq@172.28.10.14 \
  "pkill -f '/home/eloq/test-e2e/prometheusalert/PrometheusAlert' || true; \
   rm -rf /home/eloq/test-e2e/prometheusalert"
```

This cleanup is only for legacy leftovers. The new Feishu alerting chain is deployed under `/home/eloq/test-e2e/alertmanager-webhook-adapter`.

Export topology:

```sh
eloqctl export test-e2e --output /home/eloq/test-e2e-export.yaml
```

### 4. Open the monitor UIs

From the host browser:

- Grafana: `http://127.0.0.1:13301`
- Prometheus: `http://127.0.0.1:19500`
- Alertmanager: `http://127.0.0.1:19093` after `alertmanager` is installed
- Alertmanager Webhook Adapter: `http://127.0.0.1:18080` after `alertmanager` is installed

Default Grafana credentials:

```text
admin / admin
```

You can also validate the endpoints with commands:

```sh
curl -fsS http://127.0.0.1:13301/login >/dev/null
curl -fsS http://127.0.0.1:19500/-/healthy
curl -fsS http://127.0.0.1:19093/-/healthy
curl -fsS http://127.0.0.1:18080 >/dev/null
```

### 5. Tear down the environment

Run on the host:

```sh
tests/e2e/devctl.sh env-down
```

## Common stress test variables

`tests/e2e/cmd_stress_test.sh` supports these common overrides:

| Variable | Default |
|----------|---------|
| `STEPS` | `launch,cluster-update,monitor-update,eloqctl-mutate,py-stress,go-stress,ts-stress,resp-compat,remove` |
| `DURATION_SECONDS` | `300` |
| `INFO_ONLY_DURATION_SECONDS` | `300` |
| `WORKERS` | `16` |
| `INFLIGHT` | `4` |
| `KEY_COUNT` | `256` |
| `CMD_TIMEOUT` | `5` |
| `TLS_ENABLED` | `1` |
| `SKIP_DEPS` | `1` |
| `RESP_COMPAT_VERSION` | `7.0.0` |

Example:

```sh
STEPS=py-stress,go-stress,ts-stress \
  DURATION_SECONDS=15 \
  INFO_ONLY_DURATION_SECONDS=15 \
  WORKERS=4 \
  INFLIGHT=2 \
  tests/e2e/devctl.sh stress
```
