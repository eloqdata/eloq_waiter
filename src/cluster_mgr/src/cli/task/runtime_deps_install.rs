use crate::cli::ssh::SSHSession;
use crate::cli::task::task_base::{
    ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId, TaskInstance,
};
use crate::cli::util::{os_id, os_major_version};
use crate::config::config_base::DeploymentConfig;
use anyhow::bail;
use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::vec;
use tracing::info;
use users::get_current_uid;

#[derive(Clone, Debug)]
pub struct RuntimeDepsInstallation {
    install_dep_cmd: String,
    task_id: TaskId,
    config: DeploymentConfig,
}

impl RuntimeDepsInstallation {
    pub fn from_config(
        config: &DeploymentConfig,
    ) -> anyhow::Result<IndexMap<TaskId, TaskInstance>> {
        let os_name = os_id();
        let version = os_major_version();
        let deps = DeploymentConfig::load_runtime_deps_by_os(&os_name)?;
        info!("RuntimeDep from_config = {os_name} {version}");
        let  cmd_header = match os_name.as_str() {
            "ubuntu" => vec![
                "apt update", 
                "DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends"],
            "rhel" => 
                match version.as_str() {
                    "7"=> vec![
                        "yum install -y epel-release", 
                        "yum update -y", 
                        "yum install -y"],
                    "8" => vec![
                        "dnf install -y https://dl.fedoraproject.org/pub/epel/epel-release-latest-8.noarch.rpm", 
                        "/usr/bin/crb enable", 
                        "dnf install -y epel-release", 
                        "dnf update -y", 
                        "dnf install -y"],
                    "9" => vec![
                        "dnf install -y https://dl.fedoraproject.org/pub/epel/epel-release-latest-9.noarch.rpm", 
                        "/usr/bin/crb enable", 
                        "dnf install -y epel-release", 
                        "dnf update -y", 
                        "dnf install -y"],
                    _ => unreachable!()
                }
            _=> {
                bail!("For now MonographDB only run on Ubuntu or Centos7/Centos8");
            }
        };
        let cmd_header = if get_current_uid() == 0 {
            cmd_header.join(" && ")
        } else {
            cmd_header
                .iter()
                .map(|e| format!("sudo {}", e))
                .collect::<Vec<String>>()
                .join(" && ")
        };
        let install_dep_cmd = format!("{cmd_header} {}", deps.join(" "));

        let conn_user = config.connection.clone().username;
        let ssh_port = config.connection.ssh_port();
        let host_values = config.get_unique_host_list();
        let install_dep_task = host_values
            .iter()
            .map(|host_name| {
                let task_id = TaskId {
                    cmd: "run_deps".to_string(),
                    task: format!("{os_name}_install_deps"),
                    host: host_name.clone(),
                };
                (
                    task_id.clone(),
                    TaskInstance {
                        task_input: HashMap::new(),
                        task: Box::new(RuntimeDepsInstallation::new(
                            install_dep_cmd.clone(),
                            task_id,
                            config.clone(),
                        )),
                        task_host: TaskHost::Remote {
                            user: conn_user.clone(),
                            port: ssh_port as usize,
                            hosts: host_name.to_string(),
                        },
                    },
                )
            })
            .collect::<IndexMap<TaskId, TaskInstance>>();

        Ok(install_dep_task)
    }

    pub fn new(install_dep_cmd: String, task_id: TaskId, config: DeploymentConfig) -> Self {
        Self {
            install_dep_cmd,
            task_id,
            config,
        }
    }
}

#[async_trait]
impl TaskExecutor for RuntimeDepsInstallation {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        info!("execute {}", self.task_id.pretty_string());
        let ssh_session = SSHSession::from_task_host(
            task_host.clone(),
            self.config.connection.ssh_auth_key().unwrap(),
        )
        .await?;
        let (code, out) = ssh_session.execute(&self.install_dep_cmd).await?;
        ssh_session.close().await?;
        if code != 0 {
            let host = match task_host {
                TaskHost::Local => "127.0.0.1".to_owned(),
                TaskHost::Remote {
                    user: _,
                    port: _,
                    hosts,
                } => hosts,
            };
            anyhow::bail!(
                "install dependency failed on {host}, code={code}: {}: {out}",
                self.install_dep_cmd
            )
        }
        Ok(None)
    }
}
