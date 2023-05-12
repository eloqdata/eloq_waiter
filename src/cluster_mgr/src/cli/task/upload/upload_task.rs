use crate::cli::ssh::SSHCommandOption::CollectOutput;
use crate::cli::ssh::SSHSession;
use crate::cli::task::task_base::{
    CmdErr, ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId,
};
use crate::cli::task::upload::*;
use crate::config::config_base::DeploymentConfig;
use crate::task_return_value;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone)]
pub struct UploadTask {
    config: DeploymentConfig,
    task_id: TaskId,
}

impl UploadTask {
    pub fn new(config: DeploymentConfig, task_id: TaskId) -> Self {
        Self { config, task_id }
    }

    pub async fn create_remote_directory(&self, remote_task_host: TaskHost) -> anyhow::Result<()> {
        let ssh_session = SSHSession::from_task_host(
            remote_task_host,
            self.config.connection.ssh_auth_key().unwrap(),
        )
        .await?;
        let mkdir = format!("mkdir -p {}", self.config.install_dir());
        let mkdir_output = ssh_session.command(mkdir.as_str(), CollectOutput).await?;
        info!("UploadTask create remote dir complete={:?}", mkdir_output);
        ssh_session.close().await?;
        Ok(())
    }
}

#[async_trait]
impl TaskExecutor for UploadTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        remote_task_host: TaskHost,
        task_input: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        println!("{} execute.\n", self.task_id.pretty_string());
        self.create_remote_directory(remote_task_host.clone())
            .await?;

        let local_ip_addr = if let Some(source_host_value) = task_input.get(SOURCE_HOST) {
            source_host_value.clone().into_inner_value()
        } else {
            let source_ip_rs = local_ip_address::local_ip()?;
            source_ip_rs.to_string()
        };
        let ssh_port = self.config.connection.ssh_port();
        let ssh_user = self.config.connection.clone().username;
        let source_task_host = TaskHost::Remote {
            user: ssh_user,
            port: ssh_port as usize,
            hosts: local_ip_addr,
        };

        let ssh_session = SSHSession::from_task_host(
            source_task_host,
            self.config.connection.ssh_auth_key().unwrap(),
        )
        .await?;

        let remote_install_dir = self.config.install_dir();

        let (remote_user, port, remote_host) = remote_task_host.ssh_conn_tuple();
        let source_path_str =
            TaskArgValue::into_inner_value::<String>(task_input.get(SOURCE_PATH).unwrap().clone());

        let source_path_buf = PathBuf::from(source_path_str.as_str());

        let dest_file_name = if let Some(dest_file_str) = task_input.get(DEST_PATH) {
            TaskArgValue::into_inner_value::<String>(dest_file_str.clone())
        } else {
            source_path_buf
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        };

        let copy_dir = if let Some(scp_dir) = task_input.get(COPY_DIR) {
            TaskArgValue::into_inner_value::<String>(scp_dir.clone())
        } else {
            "".to_string()
        };

        let scp_auth_key = format!("-i {}", self.config.connection.ssh_auth_key().unwrap());
        // scp /xxx/local_file user@remote_host:remote_dir/file
        let scp_cmd = format!(
            // dir port, usr host remote_dir file_name
            r#"scp -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no {copy_dir} {scp_auth_key} -P {port} {source_path_str} {remote_user}@{remote_host}:{remote_install_dir}/{dest_file_name}"#,
        );
        info!("UploadTask cmd={}", scp_cmd);
        let err_msg = format!("cmd={scp_cmd},source_path={source_path_str}");
        let task_rs = ssh_session.command(scp_cmd.as_str(), CollectOutput).await?;
        ssh_session.close().await?;
        task_return_value!(
            task_rs,
            |status_code: usize| -> CmdErr { CmdErr::UploadErr(err_msg, status_code.to_string()) },
            "UploadTask"
        );
    }
}
