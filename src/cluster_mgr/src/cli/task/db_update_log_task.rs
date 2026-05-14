use crate::cli::task::task_base::{
    CmdErr, ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId,
};
use crate::config::config_base::DeployConfig;
use anyhow::anyhow;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;

/// Task for updating the saved cluster topology after log scaling operations
#[derive(Clone, Debug)]
pub struct DbDeploymentUpdateLogTask {
    task_id: TaskId,
    config: DeployConfig,
    cluster_name: String,
}

impl DbDeploymentUpdateLogTask {
    pub fn new(task_id: TaskId, config: DeployConfig, cluster_name: String) -> Self {
        Self {
            task_id,
            config,
            cluster_name,
        }
    }

    async fn update_saved_topology(&self) -> anyhow::Result<()> {
        info!(
            "Updating saved topology for cluster {} after log scaling operation",
            self.cluster_name
        );

        // The modified config should already contain the updated log service configuration
        let updated_config = self.config.clone();

        crate::state::state_mgr::STATE_MGR
            .save_deployment_config(&updated_config, true)
            .await
            .map_err(|e| anyhow!("Failed to update saved topology: {}", e))?;

        info!("Successfully updated saved cluster topology");

        Ok(())
    }
}

#[async_trait]
impl TaskExecutor for DbDeploymentUpdateLogTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        _task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        info!(
            "Executing {} to update saved topology with new log nodes",
            self.task_id.format_string()
        );

        // Update the saved topology with the new configuration.
        if let Err(err) = self.update_saved_topology().await {
            return Err(anyhow!(CmdErr::ScaleOpErr(
                "Failed to update saved deployment topology".to_string(),
                err.to_string(),
            )));
        }

        // Return success
        let response = HashMap::from([
            (
                crate::cli::CMD.to_string(),
                TaskArgValue::Str("Update saved topology with scaled log nodes".to_string()),
            ),
            (crate::cli::CMD_STATUS.to_string(), TaskArgValue::Number(0)),
            (
                crate::cli::CMD_OUTPUT.to_string(),
                TaskArgValue::Str(
                    "Database updated successfully with scaled log nodes".to_string(),
                ),
            ),
        ]);

        Ok(Some(response))
    }
}
