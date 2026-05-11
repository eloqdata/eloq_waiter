## Upgrade Command line tool self
Simply execute:
```shell
cluster_mgr update
```
And then follow the output of the `update` command to replace the CLI binary. For example: `tar -xzvf /home/eloquser/.eloqwaiter/download/waiter-rhel7-amd64.tar.gz -C /home/eloquser/.eloqwaiter --strip-components 1 --overwrite`

## Upgrade eloqsql/eloqkv
Upgrade eloqsql/eloqkv to the latest stable version:
```shell
cluster_mgr update <CLUSTER> latest
```
