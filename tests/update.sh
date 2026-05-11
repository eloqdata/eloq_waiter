#!/bin/bash
set -exo pipefail

echo ">>> Test Update command"

eloqctl demo eloq-kv --skip-deps --joint-wal --no-monitor
eloqctl update demo-kv-rocksdb latest
eloqctl remove demo-kv-rocksdb

echo "Update tests PASSED !!!"
