#!/bin/bash
set -exo pipefail

echo ">>> Test Demo command"

eloqctl --version

test eloq-kv

eloqctl demo eloq-kv --skip-deps
CLIENT=$(eloqctl -q connect demo-kv-rocksdb)
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eloqctl monitor stop demo-kv-rocksdb
eloqctl list
eloqctl stop demo-kv-rocksdb --all
eloqctl remove demo-kv-rocksdb

eloqctl demo eloq-kv --skip-deps --no-monitor
eloqctl remove demo-kv-rocksdb

eloqctl demo eloq-kv --skip-deps --joint-wal
eloqctl remove demo-kv-rocksdb

eloqctl demo eloq-kv --skip-deps --joint-wal --no-monitor
eloqctl remove demo-kv-rocksdb

echo "Demo tests PASSED !!!"
