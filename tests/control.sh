#!/bin/bash
set -exo pipefail

echo ">>> Test Start/Stop command"

eloqctl demo eloq-kv --skip-deps
eloqctl stop demo-kv-rocksdb
eloqctl start demo-kv-rocksdb
eloqctl stop demo-kv-rocksdb --tx false
eloqctl stop demo-kv-rocksdb --all
eloqctl start demo-kv-rocksdb
eloqctl remove demo-kv-rocksdb

echo "Control tests PASSED !!!"
