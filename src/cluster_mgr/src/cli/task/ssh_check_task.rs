use crate::cli::ssh::SSHSession;
use crate::cli::task::task_base::{
    ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId, TaskInstance,
};
use crate::config::config_base::DeployConfig;
use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use tracing::info;

#[derive(Clone, Debug)]
pub struct SshCheckTask {
    task_id: TaskId,
    config: DeployConfig,
}

impl SshCheckTask {
    pub fn from_hosts(
        config: &DeployConfig,
        hosts: Vec<String>,
        task_name: &str,
    ) -> IndexMap<TaskId, TaskInstance> {
        hosts
            .into_iter()
            .map(|host| {
                let task_id = TaskId {
                    cmd: "ssh-check".to_string(),
                    task: task_name.to_string(),
                    host: host.clone(),
                };
                let task = SshCheckTask {
                    task_id: task_id.clone(),
                    config: config.clone(),
                };
                (
                    task_id,
                    TaskInstance {
                        task_input: HashMap::new(),
                        task: Box::new(task),
                        task_host: TaskHost::remote(&config.connection, host),
                    },
                )
            })
            .collect()
    }
}

#[async_trait]
impl TaskExecutor for SshCheckTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        info!("execute {}", self.task_id.format_string());
        let session =
            SSHSession::from_task_host(task_host, self.config.connection.ssh_auth_key().unwrap())
                .await?;
        let (code, output) = session.execute("true").await?;
        session.close().await?;
        if code != 0 {
            anyhow::bail!("SSH check failed on {}: {output}", self.task_id.host);
        }
        Ok(None)
    }
}
