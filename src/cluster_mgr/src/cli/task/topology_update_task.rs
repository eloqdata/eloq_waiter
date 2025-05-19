use crate::cli::task::task_base::TaskExecutor;
use crate::cli::task::task_base::{ExecutionValue, TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::{CMD, CMD_OUTPUT, CMD_STATUS};
use crate::config::config_base::DeployConfig;
use crate::state::state_base::StateOperation;
use crate::state::state_mgr::{STATE_MGR, TOPOLOGY_STATE};
use crate::state::topology_operation::{TopologyEntity, TopologyOperation};
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use indexmap::IndexMap;
use std::collections::HashMap;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct TopologyUpdateTask {
    task_id: TaskId,
    config: DeployConfig,
}

impl TopologyUpdateTask {
    pub fn new(task_id: TaskId, config: DeployConfig) -> Self {
        Self { task_id, config }
    }

    pub fn from_config(config: &DeployConfig) -> IndexMap<TaskId, TaskInstance> {
        let mut executable = IndexMap::new();

        let task_id = TaskId {
            cmd: "topology-update".to_string(),
            task: "launch".to_string(),
            host: "local".to_string(),
        };

        let task = Box::new(TopologyUpdateTask::new(task_id.clone(), config.clone()));
        executable.insert(
            task_id.clone(),
            TaskInstance {
                task_input: HashMap::default(),
                task,
                task_host: TaskHost::Local,
            },
        );

        executable
    }

    // Extract topology information from the DeployConfig
    fn extract_topology(&self) -> Vec<TopologyEntity> {
        let mut topology_entities = Vec::new();
        let cluster_name = &self.config.deployment.cluster_name;
        let now = Utc::now();

        // 1. Process tx_host_ports (masters)
        let tx_hosts = &self.config.deployment.tx_service.tx_host_ports;
        for (i, host_port) in tx_hosts.iter().enumerate() {
            if let Some((host, port_str)) = host_port.split_once(':') {
                if let Ok(port) = port_str.parse::<i32>() {
                    let node_group_id = 0; // Main node group

                    topology_entities.push(TopologyEntity {
                        cluster_name: cluster_name.clone(),
                        node_group_count: tx_hosts.len() as i32,
                        node_group_id,
                        node_id: format!("node-{}", i),
                        is_candidate: false, // Main nodes are not candidates
                        is_master: true,     // These are master nodes
                        host: host.to_string(),
                        port,
                        cluster_config: None,
                        create_timestamp: now,
                        update_timestamp: now,
                    });
                }
            }
        }

        // 2. Process standby_host_ports if any (replicas)
        if let Some(standby_hosts) = &self.config.deployment.tx_service.standby_host_ports {
            for (i, host_port) in standby_hosts.iter().enumerate() {
                // Standby hosts can be in the format "host:port" or "host:port|host:port,..."
                for entry in host_port.split(['|', ',']) {
                    if let Some((host, port_str)) = entry.split_once(':') {
                        if let Ok(port) = port_str.parse::<i32>() {
                            let node_group_id = 1; // Standby node group

                            topology_entities.push(TopologyEntity {
                                cluster_name: cluster_name.clone(),
                                node_group_count: tx_hosts.len() as i32,
                                node_group_id,
                                node_id: format!("standby-{}", i),
                                is_candidate: false, // Standby nodes are not candidates
                                is_master: false,    // These are replica nodes
                                host: host.to_string(),
                                port,
                                cluster_config: None,
                                create_timestamp: now,
                                update_timestamp: now,
                            });
                        }
                    }
                }
            }
        }

        // 3. Process voter_host_ports if any (can be either masters or replicas based on configuration)
        if let Some(voter_hosts) = &self.config.deployment.tx_service.voter_host_ports {
            for (i, host_port) in voter_hosts.iter().enumerate() {
                // Voter hosts can be in the format "host:port" or "host:port|host:port,..."
                for entry in host_port.split(['|', ',']) {
                    if let Some((host, port_str)) = entry.split_once(':') {
                        if let Ok(port) = port_str.parse::<i32>() {
                            let node_group_id = 2; // Voter node group

                            topology_entities.push(TopologyEntity {
                                cluster_name: cluster_name.clone(),
                                node_group_count: tx_hosts.len() as i32,
                                node_group_id,
                                node_id: format!("voter-{}", i),
                                is_candidate: true, // Voters are candidates
                                is_master: false,   // Voters are not typically masters
                                host: host.to_string(),
                                port,
                                cluster_config: None,
                                create_timestamp: now,
                                update_timestamp: now,
                            });
                        }
                    }
                }
            }
        }

        topology_entities
    }
}

#[async_trait]
impl TaskExecutor for TopologyUpdateTask {
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

        // Extract topology from the config
        let topology_entities = self.extract_topology();

        if topology_entities.is_empty() {
            let message = format!(
                "No topology information found in the configuration for cluster {}",
                self.config.deployment.cluster_name
            );
            error!("{}", message);
            task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(1));
            task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(message));
            return Ok(Some(task_result));
        }

        // Get the topology operation from STATE_MGR
        let topology_operation = STATE_MGR.get_state_operation::<TopologyOperation>(TOPOLOGY_STATE);

        // Save each topology entity
        let mut success_count = 0;
        let mut failure_count = 0;

        for entity in topology_entities {
            match topology_operation.put(entity.clone()).await {
                Ok(_) => {
                    success_count += 1;
                    info!(
                        "Successfully updated topology for node: {}:{} in group {}",
                        entity.host, entity.port, entity.node_group_id
                    );
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "Failed to update topology for node: {}:{} in group {}: {}",
                        entity.host, entity.port, entity.node_group_id, e
                    );
                }
            }
        }

        let output = format!(
            "Topology update completed for cluster {}. Successfully updated {} nodes, {} failures.",
            self.config.deployment.cluster_name, success_count, failure_count
        );

        info!("{}", output);

        let status = if failure_count > 0 { 1 } else { 0 };
        task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(status));
        task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(output));

        Ok(Some(task_result))
    }
}
