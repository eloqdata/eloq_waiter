#!/bin/bash
NOW=$(date +"%Y-%m-%d-%H:%M:%S:%6N")
function log_start() {
  log_dir=${LOG_INSTALL_DIR}/logs/lg${GROUP_ID}/ln${NODE_ID}
  mkdir -p ${STORAGE_DIR} && mkdir -p ${log_dir}
  export ASAN_OPTIONS=${ADD_ASAN_OPTS}:log_path=${log_dir}/asan
  export LD_PRELOAD=${LOG_INSTALL_DIR}/lib/libmimalloc.so.2
  export LD_LIBRARY_PATH=${LOG_INSTALL_DIR}/lib:${LD_LIBRARY_PATH}
  log_start_cmd="${LOG_INSTALL_DIR}/bin/launch_sv -conf=${GROUP_MEMBERS} -raft_max_parallel_append_entries_rpc_num=64 \
    -raft_enable_append_entries_cache=true -raft_max_append_entries_cache_size=256 \
    -start_log_group_id=${GROUP_ID} -node_id=${NODE_ID} -storage_path=${STORAGE_DIR} > ${log_dir}/log_start_${NOW}.log 2>&1 &"
  echo "$log_start_cmd"
  eval "$log_start_cmd"
}
log_start