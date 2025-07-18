use crate::cli::task::monograph_log_ctl_task::MonographLogCtlTask;
use crate::cli::task::task_base::{
    CmdErr, ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId,
};
use crate::cli::{SubCommand, CMD, CMD_OUTPUT, CMD_STATUS};
use crate::config::config_base::DeployConfig;
use crate::task_return_value;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::watch;
use tracing::{error, info, warn};

#[derive(Clone, Debug)]
pub struct ScaleLogCleanupTask {
    task_id: TaskId,
    nodes: Vec<String>,
    config: DeployConfig,
    scale_result_rx: watch::Receiver<bool>,
}

impl ScaleLogCleanupTask {
    pub fn new(
        task_id: TaskId,
        nodes: Vec<String>,
        config: DeployConfig,
        scale_result_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            task_id,
            nodes,
            config,
            scale_result_rx,
        }
    }

    /// Stop the newly started log services when AddPeer operation fails
    async fn stop_newly_started_log_services(&self) -> Result<()> {
        info!("Stopping newly started log services due to AddPeer operation failure");

        // Create a temporary configuration with only the nodes that need to be stopped
        let mut temp_config = self.config.clone();
        if let Some(log_service) = &mut temp_config.deployment.log_service {
            // Filter the nodes to only include the ones that were added
            log_service.nodes.retain(|node| {
                let node_addr = format!("{}:{}", node.host, node.port);
                self.nodes.contains(&node_addr)
            });
        }

        // Use MonographLogCtlTask to stop the nodes properly
        let stop_cmd = SubCommand::Stop {
            cluster: temp_config.deployment.cluster_name.clone(),
            tx: Some(false),
            log: true,
            store: false,
            monitor: false,
            force: false,
            all: false,
            password: None,
            nodes: Vec::new(),
        };

        let stop_tasks = MonographLogCtlTask::from_config(stop_cmd, &temp_config);

        if stop_tasks.is_empty() {
            warn!("No log services to stop");
            return Ok(());
        }

        // Execute all stop tasks
        for (task_id, instance) in stop_tasks {
            info!("Executing cleanup stop task: {}", task_id.format_string());
            match instance
                .task
                .execute(instance.task_host, instance.task_input)
                .await
            {
                Ok(_) => info!("Successfully stopped log service on {}", task_id.host),
                Err(e) => error!("Failed to stop log service on {}: {}", task_id.host, e),
            }
        }

        info!("Completed cleanup of newly started log services");
        Ok(())
    }
}

#[async_trait]
impl TaskExecutor for ScaleLogCleanupTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        _task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> Result<Option<ExecutionValue>> {
        let mut task_result = HashMap::from([(
            CMD.to_string(),
            TaskArgValue::Str("scale log cleanup operation".to_string()),
        )]);

        info!(
            "Executing scale log cleanup operation for nodes: {:?}",
            self.nodes
        );

        // Check if the previous scale operation failed by reading from the channel
        let previous_failed = !*self.scale_result_rx.borrow();
        info!("Scale operation result from channel: {}", !previous_failed);

        if !previous_failed {
            // Previous task succeeded, no cleanup needed
            info!("Previous scale operation succeeded, no cleanup needed");
            task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(0));
            task_result.insert(
                CMD_OUTPUT.to_string(),
                TaskArgValue::Str("Previous operation succeeded, no cleanup performed".to_string()),
            );
            return Ok(Some(task_result));
        }

        // Previous task failed, perform cleanup
        info!("Previous scale operation failed, performing cleanup");
        match self.stop_newly_started_log_services().await {
            Ok(_) => {
                task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(0));
                task_result.insert(
                    CMD_OUTPUT.to_string(),
                    TaskArgValue::Str(format!(
                        "Successfully cleaned up log services for nodes: {}",
                        self.nodes.join(", ")
                    )),
                );
            }
            Err(e) => {
                let error_msg = format!("Failed to cleanup log services: {}", e);
                error!("{}", error_msg);
                task_result.insert(CMD_STATUS.to_string(), TaskArgValue::Number(1));
                task_result.insert(CMD_OUTPUT.to_string(), TaskArgValue::Str(error_msg.clone()));
                task_return_value!(
                    task_result,
                    |status_code: i32| -> CmdErr {
                        CmdErr::ScaleOpErr(error_msg, status_code.to_string())
                    },
                    "ScaleLogCleanupTask"
                )
            }
        }

        Ok(Some(task_result))
    }
}
