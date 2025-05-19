use crate::cli::task::task_base::TaskExecutor;
use crate::cli::task::task_base::{ExecutionValue, TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::SubCommand;
use crate::cli::{CMD, CMD_OUTPUT, CMD_STATUS};
use crate::state::state_mgr::STATE_MGR;
use anyhow::Result;
use async_trait::async_trait;
use indexmap::IndexMap;
use prettytable::{format, row, Cell, Row, Table};
use std::collections::HashMap;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct TopologyDisplayTask {
    task_id: TaskId,
    cluster_name: String,
}

impl TopologyDisplayTask {
    pub fn new(task_id: TaskId, cluster_name: String) -> Self {
        Self {
            task_id,
            cluster_name,
        }
    }

    pub fn from_command(command: SubCommand) -> IndexMap<TaskId, TaskInstance> {
        let mut executable = IndexMap::new();

        if let SubCommand::Status {
            cluster, detail, ..
        } = command
        {
            if detail {
                let task_id = TaskId {
                    cmd: "topology-display".to_string(),
                    task: "status".to_string(),
                    host: "local".to_string(),
                };
                let task = Box::new(TopologyDisplayTask::new(task_id.clone(), cluster));
                executable.insert(
                    task_id.clone(),
                    TaskInstance {
                        task_input: HashMap::default(),
                        task,
                        task_host: TaskHost::Local,
                    },
                );
            }
        }

        executable
    }
}

#[async_trait]
impl TaskExecutor for TopologyDisplayTask {
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
            TaskArgValue::Str("topology-display".to_string()),
        );

        info!(
            "Displaying topology information for cluster: {}",
            self.cluster_name
        );

        match STATE_MGR
            .load_topology_from_state(self.cluster_name.clone())
            .await
        {
            Ok(topology_entities) => {
                if topology_entities.is_empty() {
                    let message = format!(
                        "No topology information found for cluster {}",
                        self.cluster_name
                    );
                    info!("{}", message);
                    task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(0));
                    task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(message));
                    return Ok(Some(task_result));
                }

                // Create a table for the output
                let mut table = Table::new();
                table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

                // Add table header
                table.set_titles(row![
                    "Node Group ID",
                    "Node ID",
                    "Host",
                    "Port",
                    "Role",
                    "Is Candidate"
                ]);

                // Group by node_group_id for better organization
                let mut grouped: HashMap<i32, Vec<_>> = HashMap::new();
                let entities_for_display = topology_entities.clone();
                for entity in entities_for_display {
                    grouped
                        .entry(entity.node_group_id)
                        .or_insert_with(Vec::new)
                        .push(entity);
                }

                // Sort by node_group_id for consistent display
                let mut group_ids: Vec<i32> = grouped.keys().cloned().collect();
                group_ids.sort();

                // Build the rows in the table
                for group_id in group_ids {
                    if let Some(nodes) = grouped.get(&group_id) {
                        for node in nodes {
                            let role = if node.is_master { "Master" } else { "Replica" };
                            let candidate = if node.is_candidate { "Yes" } else { "No" };

                            table.add_row(Row::new(vec![
                                Cell::new(&node.node_group_id.to_string()),
                                Cell::new(&node.node_id),
                                Cell::new(&node.host),
                                Cell::new(&node.port.to_string()),
                                Cell::new(role),
                                Cell::new(candidate),
                            ]));
                        }
                    }
                }

                // Convert the table to a string and store it in the task result
                let table_string = format!(
                    "\nCluster Topology for {}:\n{}",
                    self.cluster_name,
                    table.to_string()
                );
                info!("Successfully displayed topology information");

                task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(0));
                task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(table_string));
            }
            Err(e) => {
                let error_msg = format!("Failed to load topology information: {}", e);
                error!("{}", error_msg);
                task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(1));
                task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(error_msg));
            }
        }

        Ok(Some(task_result))
    }
}
