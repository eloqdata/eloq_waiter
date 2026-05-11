#!/bin/bash
set -exo pipefail

echo ">>> Test Launch command"

MY_IP=$(ip -4 addr | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | sed -n '2p')
sed -i "s|127.0.0.1|${MY_IP}|g" ${ELOQCTL_HOME}/config/examples/eloqkv_rocksdb.yaml

eloqctl launch ${ELOQCTL_HOME}/config/examples/eloqkv_rocksdb.yaml -s
CLIENT=$(eloqctl -q connect eloqkv-cluster)
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eloqctl stop eloqkv-cluster --all
eloqctl inspect eloqkv-cluster

eloqctl list
eloqctl remove eloqkv-cluster

echo "Launch tests PASSED !!!"
