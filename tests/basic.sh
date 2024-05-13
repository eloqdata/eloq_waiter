#!/bin/bash
set -exo pipefail

echo ">>> Test Demo command"

# test eloq-sql
cluster_mgr demo --product eloq-sql --version nightly
CLIENT=$(cluster_mgr connect --cluster demo-sql-cassandra)
cluster_mgr status --cluster demo-sql-cassandra --wait 5
eval "${CLIENT} --execute 'SHOW DATABASES'"
cluster_mgr monitor --command stop --cluster demo-sql-cassandra
cluster_mgr stop --cluster demo-sql-cassandra
cluster_mgr update-conf --cluster demo-sql-cassandra
cluster_mgr start --cluster demo-sql-cassandra
cluster_mgr status --cluster demo-sql-cassandra --wait 5
eval "${CLIENT} --execute 'SHOW DATABASES'"
cluster_mgr stop --cluster demo-sql-cassandra --all

# test eloq-kv
sleep 15
cluster_mgr demo --product eloq-kv --version nightly
CLIENT=$(cluster_mgr connect --cluster demo-kv-cassandra)
cluster_mgr status --cluster demo-kv-cassandra --wait 5
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
cluster_mgr monitor --command stop --cluster demo-kv-cassandra
cluster_mgr list
cluster_mgr stop --cluster demo-kv-cassandra --all

sleep 15
cluster_mgr demo --product eloq-kv --store rocks
CLIENT=$(cluster_mgr connect --cluster demo-kv-rocksdb)
cluster_mgr status --cluster demo-kv-rocksdb --wait 5
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
cluster_mgr monitor --command stop --cluster demo-kv-rocksdb
cluster_mgr list
cluster_mgr stop --cluster demo-kv-rocksdb --all

echo ">>> Test Launch command"

sleep 15
cluster_mgr launch --topology-file ${CLUSTER_MGR_HOME}/config/deployment_sql.yaml
CLIENT=$(cluster_mgr connect --cluster eloqsql-cluster)
cluster_mgr status --cluster eloqsql-cluster --wait 5
eval "${CLIENT} --execute 'SHOW DATABASES'"
cluster_mgr monitor --command stop --cluster eloqsql-cluster
cluster_mgr stop --cluster eloqsql-cluster --all
cluster_mgr inspect --cluster eloqsql-cluster

sleep 15
cluster_mgr launch --topology-file ${CLUSTER_MGR_HOME}/config/deployment_kv.yaml
CLIENT=$(cluster_mgr connect --cluster eloqkv-cluster)
cluster_mgr status --cluster eloqkv-cluster --wait 5
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
cluster_mgr monitor --command stop --cluster eloqkv-cluster
cluster_mgr stop --cluster eloqkv-cluster --all
cluster_mgr inspect --cluster eloqkv-cluster

cluster_mgr list
cluster_mgr remove --cluster demo-sql-cassandra
cluster_mgr remove --cluster demo-kv-cassandra
cluster_mgr remove --cluster demo-kv-rocksdb
cluster_mgr remove --cluster eloqsql-cluster
cluster_mgr remove --cluster eloqkv-cluster
cluster_mgr list

MY_IP=$(ip -4 addr | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | sed -n '2p')
sed -i "s|127.0.0.1|${MY_IP}|g" ${CLUSTER_MGR_HOME}/config/deployment_sql.yaml
sed -i "s|127.0.0.1|${MY_IP}|g" ${CLUSTER_MGR_HOME}/config/deployment_kv.yaml

sleep 15
cluster_mgr launch --topology-file ${CLUSTER_MGR_HOME}/config/deployment_sql.yaml
CLIENT=$(cluster_mgr connect --cluster eloqsql-cluster)
cluster_mgr status --cluster eloqsql-cluster --wait 5
eval "${CLIENT} --execute 'SHOW DATABASES'"
cluster_mgr monitor --command stop --cluster eloqsql-cluster
cluster_mgr stop --cluster eloqsql-cluster --all
cluster_mgr inspect --cluster eloqsql-cluster

sleep 15
cluster_mgr launch --topology-file ${CLUSTER_MGR_HOME}/config/deployment_kv.yaml
CLIENT=$(cluster_mgr connect --cluster eloqkv-cluster)
cluster_mgr status --cluster eloqkv-cluster --wait 5
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
eval ${CLIENT} incr mycounter
eval ${CLIENT} get mycounter
cluster_mgr monitor --command stop --cluster eloqkv-cluster
cluster_mgr stop --cluster eloqkv-cluster --all
cluster_mgr inspect --cluster eloqkv-cluster

cluster_mgr list
cluster_mgr remove --cluster eloqsql-cluster
cluster_mgr remove --cluster eloqkv-cluster

echo "Basic tests PASSED !!!"
