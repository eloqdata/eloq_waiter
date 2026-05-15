use crate::cli::task::task_base::{
    ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId, TaskInstance,
};
use crate::cli::upload_dir;
use crate::config::config_base::DeployConfig;
use crate::config::connection::Connection;
use anyhow::bail;
use std::collections::HashMap;
use tokio::time::{timeout, Duration};
use tracing::info;

#[derive(Debug, Clone)]
pub struct CopyTask {
    id: TaskId,
    conn: Connection,
    src_host: String,
    src_path: String,
    dst_path: String,
}

impl CopyTask {
    pub fn new(
        id: TaskId,
        conn: Connection,
        src_host: String,
        src_path: String,
        dst_path: String,
    ) -> Self {
        Self {
            id,
            conn,
            src_host,
            src_path,
            dst_path,
        }
    }

    pub fn fetch_datafarm(deploy: &DeployConfig) -> (TaskId, TaskInstance) {
        let id = TaskId {
            cmd: "install".to_owned(),
            task: "fetch_datafarm".to_owned(),
            host: "_NONE".to_owned(),
        };
        let boot_node = deploy
            .deployment
            .bootstrap_host()
            .expect("bootstrap host configuration is missing");
        let src_path = format!("{}/datafarm", deploy.deployment.tx_srv_home());
        let dst_path = upload_dir().to_string_lossy().to_string();
        let copy = Self::new(
            id.clone(),
            deploy.connection.clone(),
            boot_node,
            src_path,
            dst_path,
        );
        let task = TaskInstance {
            task_input: HashMap::default(),
            task: Box::new(copy),
            task_host: TaskHost::Local,
        };
        (id, task)
    }
}

#[async_trait::async_trait]
impl TaskExecutor for CopyTask {
    fn identifier(&self) -> TaskId {
        self.id.clone()
    }

    async fn execute(
        &self,
        _task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        let (ssh_host, ssh_port) = self.conn.ssh_endpoint(&self.src_host);
        let source = format!("{}@{}:{}", self.conn.username, ssh_host, self.src_path);
        let mut cmd = tokio::process::Command::new("scp");
        cmd.args([
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-i",
            &self.conn.ssh_auth_key().unwrap(),
            "-P",
            &ssh_port.to_string(),
            "-r",
            &source,
            &self.dst_path,
        ]);
        let out = timeout(Duration::from_secs(120), cmd.output()).await??;
        info!("CopyTask {source} -> {}:\n{:#?}", self.dst_path, out);
        if !out.status.success() {
            let command_output = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            bail!(
                "CopyTask {source} -> {} failed with {:?}: {}",
                self.dst_path,
                out.status.code(),
                command_output.trim()
            );
        }
        Ok(None)
    }
}
