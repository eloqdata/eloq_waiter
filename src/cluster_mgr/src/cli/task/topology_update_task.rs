use crate::cli::task::redis_op_task::ClusterNodes;
use crate::cli::task::task_base::TaskExecutor;
use crate::cli::task::task_base::{ExecutionValue, TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::task::task_utils::parse_ng_config;
use crate::cli::{CMD, CMD_OUTPUT, CMD_STATUS};
use crate::config::config_base::DeployConfig;
use crate::state::state_base::{QueryCondition, StateOperation};
use crate::state::state_mgr::{STATE_MGR, TOPOLOGY_LOG_STATE, TOPOLOGY_TX_STATE};
use crate::state::topology_log_operation::{TopologyLogEntity, TopologyLogOperation};
use crate::state::topology_tx_operation::{TopologyTxEntity, TopologyTxOperation};
use crate::StateValue;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use indexmap::IndexMap;
use std::collections::HashMap;
use tokio::sync::watch;
use tracing::{error, info};

// Update topology in t_topology_tx using live data from RedisOpTask
#[derive(Debug, Clone)]
pub struct TopologyUpdateFromRedisTask {
    task_id: TaskId,
    cluster_name: String,
    config: DeployConfig,
    receiver: watch::Receiver<ClusterNodes>,
}

impl TopologyUpdateFromRedisTask {
    /// Create tasks to update topology using data from a RedisOpTask channel
    pub fn from_redis(
        config: &DeployConfig,
        receiver: watch::Receiver<ClusterNodes>,
    ) -> IndexMap<TaskId, TaskInstance> {
        let mut map = IndexMap::new();
        let task_id = TaskId {
            cmd: "topology-update".to_string(),
            task: "redis".to_string(),
            host: "local".to_string(),
        };
        let task = Box::new(TopologyUpdateFromRedisTask {
            task_id: task_id.clone(),
            cluster_name: config.deployment.cluster_name.clone(),
            config: config.clone(),
            receiver,
        });
        map.insert(
            task_id.clone(),
            TaskInstance {
                task_input: HashMap::new(),
                task,
                task_host: TaskHost::Local,
            },
        );
        map
    }

    // Extract TX entries from node group config parsed by parse_ng_config
    fn extract_tx_topology(&self) -> Vec<TopologyTxEntity> {
        let mut tx_entities = Vec::new();
        let port_delta = 10000;
        let cluster_name = &self.config.deployment.cluster_name;
        let now = Utc::now();

        // Get the configuration strings from the deployment config
        let tx_ip_port_list = self.config.deployment.tx_service.tx_host_ports.join(",");
        let standby_ip_port_list = self
            .config
            .deployment
            .tx_service
            .standby_host_ports
            .as_ref()
            .map_or("".to_string(), |hosts| hosts.join(","));
        let voter_ip_port_list = self
            .config
            .deployment
            .tx_service
            .voter_host_ports
            .as_ref()
            .map_or("".to_string(), |hosts| hosts.join(","));

        // Parse node group configuration
        match parse_ng_config(
            &tx_ip_port_list,
            &standby_ip_port_list,
            &voter_ip_port_list,
            Some(port_delta),
        ) {
            Ok(ng_configs) => {
                let node_group_count = ng_configs.len() as i32;

                // Process each node group
                for (ng_id, nodes) in ng_configs.iter() {
                    // Process each node in the group
                    for node in nodes {
                        // Determine the role based on is_candidate flag
                        // 1 = Replica (Standby), 2 = Voter
                        // All is_candidate=true nodes are set as replicas at this step
                        let role = if node.is_candidate {
                            1 // All candidates are replicas initially
                        } else {
                            2 // Voter
                        };

                        tx_entities.push(TopologyTxEntity {
                            cluster_name: cluster_name.clone(),
                            node_group_count,
                            node_group_id: *ng_id as i32,
                            node_id: format!("{}", node.node_id),
                            role,
                            host: node.ip.clone(),
                            port: (node.port as i32 - port_delta as i32),
                            create_timestamp: now,
                            update_timestamp: now,
                        });
                    }
                }

                info!(
                    "Parsed node group configuration with {} groups and {} nodes",
                    node_group_count,
                    tx_entities.len()
                );
            }
            Err(err) => {
                error!("Failed to parse node group configuration: {}", err);

                // Fall back to older method for voters only
                if let Some(voter_hosts) = &self.config.deployment.tx_service.voter_host_ports {
                    for (i, host_port) in voter_hosts.iter().enumerate() {
                        for entry in host_port.split(['|', ',']) {
                            if let Some((host, port_str)) = entry.split_once(':') {
                                if let Ok(port) = port_str.parse::<i32>() {
                                    tx_entities.push(TopologyTxEntity {
                                        cluster_name: cluster_name.clone(),
                                        node_group_count: 0,
                                        node_group_id: 0,
                                        node_id: format!("voter-{}", i),
                                        role: 2,
                                        host: host.to_string(),
                                        port,
                                        create_timestamp: now,
                                        update_timestamp: now,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        tx_entities
    }

    // Extract log service entries from DeployConfig
    fn extract_log_topology(&self) -> Vec<TopologyLogEntity> {
        let mut log_entities = Vec::new();
        let cluster_name = &self.config.deployment.cluster_name;
        let now = Utc::now();
        if let Some(log_service) = &self.config.deployment.log_service {
            let log_nodes = &log_service.nodes;
            let log_count = log_nodes.len() as i32;
            for (i, node) in log_nodes.iter().enumerate() {
                log_entities.push(TopologyLogEntity {
                    cluster_name: cluster_name.clone(),
                    node_group_count: log_count,
                    node_group_id: i as i32,
                    node_id: format!("log-{}", i),
                    host: node.host.clone(),
                    port: node.port as i32,
                    data_dirs: Some(node.data_dir.join(",")),
                    create_timestamp: now,
                    update_timestamp: now,
                });
            }
        }
        log_entities
    }
}

#[async_trait]
impl TaskExecutor for TopologyUpdateFromRedisTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        _task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> Result<Option<ExecutionValue>> {
        let mut task_result = HashMap::new();
        task_result.insert(
            CMD.to_string(),
            TaskArgValue::Str("topology-update".to_string()),
        );

        info!(
            "Updating topology information for cluster: {}",
            self.config.deployment.cluster_name
        );

        let now = Utc::now();

        let tx_op = STATE_MGR.get_state_operation::<TopologyTxOperation>(TOPOLOGY_TX_STATE);
        let log_op = STATE_MGR.get_state_operation::<TopologyLogOperation>(TOPOLOGY_LOG_STATE);

        let mut success_count = 0;
        let mut failure_count = 0;

        // Use the new extract_tx_topology method that uses parse_ng_config
        let tx_entries = self.extract_tx_topology();
        for tx_entity in tx_entries {
            match tx_op.put(tx_entity.clone()).await {
                Ok(_) => {
                    success_count += 1;
                    info!(
                        "Updated TX node: {}:{} role {} in group {}",
                        tx_entity.host, tx_entity.port, tx_entity.role, tx_entity.node_group_id
                    );
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "Failed TX update for node {}:{} group {}: {}",
                        tx_entity.host, tx_entity.port, tx_entity.node_group_id, e
                    );
                }
            }
        }

        // Update log entries
        let log_entries = self.extract_log_topology();
        for log_entity in log_entries {
            match log_op.put(log_entity.clone()).await {
                Ok(_) => {
                    success_count += 1;
                    info!(
                        "Updated LOG node: {}:{} in group {}",
                        log_entity.host, log_entity.port, log_entity.node_group_id
                    );
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "Failed LOG update for node {}:{} group {}: {}",
                        log_entity.host, log_entity.port, log_entity.node_group_id, e
                    );
                }
            }
        }

        let mut status_rx = self.receiver.clone();

        // Wait for RedisOpTask to send the updated cluster nodes
        if let Err(e) = status_rx.changed().await {
            let msg = format!("Failed to receive cluster nodes from channel: {}", e);
            error!("{}", msg);
            task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(1));
            task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(msg));
            return Ok(Some(task_result));
        }
        let cluster_nodes = self.receiver.borrow().clone();

        // Update master roles for nodes identified as masters from Redis
        let master_count = cluster_nodes.masters.len() as i32;
        for master in cluster_nodes.masters.iter() {
            // First, query for existing entry with the same host:port (should be a replica)
            let master_ip = master.ip.clone();
            let master_port = master.port as i32;

            // Use load with a closure that returns Option<QueryCondition>
            match tx_op
                .load(|| {
                    let cond_text = "host = ? and port = ? and role = ?".to_string();
                    let bind_values = vec![
                        StateValue::Varchar(master_ip.clone()),
                        StateValue::Integer(master_port),
                        StateValue::Integer(1), // role = 1 (replica)
                    ];
                    Some(QueryCondition {
                        cond_text,
                        bind_values,
                    })
                })
                .await
            {
                Ok(existing_entities) => {
                    if !existing_entities.is_empty() {
                        // Found matching replica(s), update role to master (0)
                        for mut entity in existing_entities {
                            entity.role = 0; // Change role from replica to master
                            entity.update_timestamp = now;

                            match tx_op.put(entity.clone()).await {
                                Ok(_) => {
                                    success_count += 1;
                                    info!(
                                        "Updated role to master for node {}:{} in group {}",
                                        entity.host, entity.port, entity.node_group_id
                                    );
                                }
                                Err(e) => {
                                    failure_count += 1;
                                    error!(
                                        "Failed to update role to master for node {}:{} in group {}: {}",
                                        entity.host, entity.port, entity.node_group_id, e
                                    );
                                }
                            }
                        }
                    } else {
                        info!(
                            "No matching replica found for master {}:{}, skipping role update",
                            master.ip, master.port
                        );
                    }
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "Failed to query topology for master {}:{}: {}",
                        master.ip, master.port, e
                    );
                }
            }
        }

        let output = format!(
            "Topology update from Redis completed for cluster {}. Updated {} entries, {} failures.",
            self.cluster_name, success_count, failure_count
        );
        info!("{}", output);
        let status = if failure_count > 0 { 1 } else { 0 };
        task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(status));
        task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(output));
        Ok(Some(task_result))
    }
}
