use crate::cli::config::DeploymentConfig;
use crate::cli::task::task_base::{
    ExecutionResult, TaskExecutionContext, TaskExecutor, TaskHost, TaskId, TaskValue,
};
use crate::ssh_conn_info;
use async_trait::async_trait;
use itertools::Itertools;
use std::collections::HashMap;
use tracing::info;

pub(crate) const REMOTE_TAR: &str = "remote_tar";

#[derive(Clone)]
pub struct UnpackFileTask {
    config: DeploymentConfig,
    task_host: TaskHost,
    task_id: TaskId,
}

impl UnpackFileTask {
    pub fn from_config(config: &DeploymentConfig) -> anyhow::Result<Vec<TaskExecutionContext>> {
        let remote_install_dir = config.install_dir();
        let conn_usr = config.connection.clone().username;
        let ssh_port = config.connection.ssh_port();
        // key is file name , value is host list
        let all_hosts = config.unpack_files_map();
        let unpack_execution_vec = all_hosts
            .into_iter()
            .map(|entry| {
                let unpack_file = entry.0;
                let hosts = entry.1;
                hosts
                    .into_iter()
                    .map(|remote_host| {
                        let remote_tarball =
                            format!("{}/{}", remote_install_dir, unpack_file.as_str());
                        let task_host = TaskHost::Remote {
                            user: conn_usr.clone(),
                            port: ssh_port as usize,
                            hosts: remote_host,
                        };
                        TaskExecutionContext {
                            task_input: HashMap::from([(
                                REMOTE_TAR.to_string(),
                                TaskValue::Str(remote_tarball),
                            )]),
                            task: Box::new(UnpackFileTask {
                                task_host: task_host.clone(),
                                config: config.clone(),
                                task_id: TaskId {
                                    cmd: "deploy".to_string(),
                                    task: format!("{}_unpack", unpack_file),
                                },
                            }),
                            task_host,
                        }
                    })
                    .collect_vec()
            })
            .into_iter()
            .flatten()
            .collect_vec();
        Ok(unpack_execution_vec)
    }
}

#[async_trait]
impl TaskExecutor for UnpackFileTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        task_input: HashMap<String, TaskValue>,
    ) -> anyhow::Result<Option<ExecutionResult>> {
        ssh_conn_info! {
            self.config.connection.clone(),
            task_host,
            ssh_conn,
            _conn_user,
            _conn_host
        }
        let remote_tar =
            TaskValue::into_inner_value::<String>(task_input.get(REMOTE_TAR).unwrap().clone());
        let install_dir = self.config.install_dir();
        let extract_cmd = if remote_tar.contains("monograph") {
            format!(
                r#"mkdir -p {}/monographdb-release && tar -zxvf {} -C {}/monographdb-release"#,
                install_dir, remote_tar, install_dir
            )
        } else {
            format!(
                r#"tar -zxvf {} -C {} && mv {} {}/apache-cassandra"#,
                remote_tar,
                install_dir,
                remote_tar.replace("-bin.tar.gz", ""),
                install_dir
            )
        };
        info!("UnpackFileTask will be start cmd={}", extract_cmd.as_str());
        let task_rs = ssh_conn?.run_cmd(extract_cmd.clone(), false);
        info!(
            "UnpackFileTask complete cmd={}, result={:?}",
            extract_cmd, task_rs
        );
        Ok(None)
    }
}
