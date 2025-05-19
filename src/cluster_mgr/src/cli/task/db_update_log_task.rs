use crate::cli::task::task_base::{
    CmdErr, ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId,
};
use crate::config::config_base::DeployConfig;
use crate::state::deployment_operation::{DeploymentEntity, DeploymentOperation};
use crate::state::state_base::{QueryCondition, StateOperation};
use crate::state::state_mgr::{DEPLOYMENT_STATE, STATE_MGR};
use crate::StateValue;
use anyhow::anyhow;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;

/// Task for updating the database after log scaling operations
#[derive(Clone, Debug)]
pub struct DbDeploymentUpdateLogTask {
    task_id: TaskId,
    config: DeployConfig,
    log_ng_id: Option<u32>,
    cluster_name: String,
}

impl DbDeploymentUpdateLogTask {
    pub fn new(
        task_id: TaskId,
        config: DeployConfig,
        log_ng_id: Option<u32>,
        cluster_name: String,
    ) -> Self {
        Self {
            task_id,
            config,
            log_ng_id,
            cluster_name,
        }
    }

    async fn update_database(&self) -> anyhow::Result<()> {
        info!(
            "Updating database for cluster {} after log scaling operation",
            self.cluster_name
        );

        // The modified config should already contain the updated log service configuration
        let updated_config = self.config.clone();

        // Regenerate the unique host list after updating the configuration
        let all_hosts = updated_config.get_unique_host_list().join(";");

        // Generate the updated config as YAML for storage
        let config_string = updated_config.to_yaml();
        info!("Generated updated deployment config YAML with scaled log nodes");

        // Use timestamps for the deployment entity
        let now = chrono::Utc::now();

        // Create the deployment entity for database update
        let updated_entity = DeploymentEntity {
            cluster_name: self.cluster_name.clone(),
            deployment_config: config_string,
            host_list: all_hosts,
            create_timestamp: now.into(),
            update_timestamp: now.into(),
        };

        info!("Created deployment entity for database update");

        // Get the deployment operation implementation from the global state manager
        let deployment_operation =
            STATE_MGR.get_state_operation::<DeploymentOperation>(DEPLOYMENT_STATE);

        // Query existing deployment to preserve create_timestamp if it exists
        let deployment_entity = deployment_operation
            .load(|| -> Option<QueryCondition> {
                Some(QueryCondition {
                    cond_text: "cluster_name = $1".to_string(),
                    bind_values: vec![StateValue::Varchar(self.cluster_name.clone())],
                })
            })
            .await
            .map_err(|e| anyhow!("Failed to query deployment: {}", e))?;

        if !deployment_entity.is_empty() {
            // Preserve the original creation timestamp
            let mut updated_entity_with_original_timestamp = updated_entity.clone();
            updated_entity_with_original_timestamp.create_timestamp =
                deployment_entity[0].create_timestamp;

            // Update the deployment entity in the database
            let rows_affected = deployment_operation
                .put(updated_entity_with_original_timestamp)
                .await
                .map_err(|e| anyhow!("Failed to update deployment in database: {}", e))?;

            info!(
                "Successfully updated existing deployment in database. Rows affected: {}",
                rows_affected
            );
        } else {
            // No existing entity found, insert as new
            let rows_affected = deployment_operation
                .put(updated_entity)
                .await
                .map_err(|e| anyhow!("Failed to insert new deployment in database: {}", e))?;

            info!(
                "Successfully inserted new deployment in database. Rows affected: {}",
                rows_affected
            );
        }

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
            "Executing {} to update database with new log nodes",
            self.task_id.format_string()
        );

        // Update the database with the new configuration
        if let Err(err) = self.update_database().await {
            return Err(anyhow!(CmdErr::ScaleOpErr(
                "Failed to update deployment configuration in database".to_string(),
                err.to_string(),
            )));
        }

        // Return success
        let response = HashMap::from([
            (
                crate::cli::CMD.to_string(),
                TaskArgValue::Str("Update database with scaled log nodes".to_string()),
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
