use crate::cli::config::DeploymentConfig;
use crate::cli::task::ssh_conn::{SSH_EXEC_CMD_OUTPUT, SSH_EXEC_CMD_STATUS};
use crate::cli::task::task_base::{
    CmdErr, ExecutionResult, TaskExecutionContext, TaskExecutor, TaskHost, TaskId, TaskValue,
};
use crate::cli::task::upload_task::SOURCE_PATH;
use crate::cli::MONOGRAPH_INSTALL_SCRIPT;
use crate::ssh_conn_info;
use anyhow::anyhow;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{error, info};

#[derive(Clone)]
pub struct MonographInstall {
    config: DeploymentConfig,
    task_id: TaskId,
}

impl MonographInstall {
    pub fn from_config(
        config: &DeploymentConfig,
        task_host: TaskHost,
    ) -> Vec<TaskExecutionContext> {
        vec![TaskExecutionContext {
            task_input: HashMap::default(),
            task: Box::new(MonographInstall::new(
                config.clone(),
                TaskId {
                    cmd: "install".to_string(),
                    task: "monograph-install".to_string(),
                },
            )),
            task_host,
        }]
    }

    pub fn new(config: DeploymentConfig, task_id: TaskId) -> Self {
        Self { config, task_id }
    }
}

#[async_trait]
impl TaskExecutor for MonographInstall {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        _task_arg: HashMap<String, TaskValue>,
    ) -> anyhow::Result<Option<ExecutionResult>> {
        ssh_conn_info! {
            self.config.connection.clone(),
            task_host,
            ssh_conn,
            _conn_user,
            _conn_host
        }

        let remote_install_dir = self.config.install_dir();
        let install_db_script = format!(
            r#"export LD_LIBRARY_PATH={}/monographdb-release/install/lib:$LD_LIBRARY_PATH; /bin/bash {}/{}"#,
            remote_install_dir.as_str(),
            remote_install_dir.as_str(),
            MONOGRAPH_INSTALL_SCRIPT
        );
        let install_rs = ssh_conn?.run_cmd(install_db_script.clone(), true)?;
        let status_code_value = install_rs.get(SSH_EXEC_CMD_STATUS).unwrap();
        info!(
            "MonographInstall install database cmd={},status_code = {:?}",
            install_db_script, status_code_value
        );
        let status_code = TaskValue::into_inner_value::<usize>(status_code_value.clone());
        let install_db_output = install_rs.get(SSH_EXEC_CMD_OUTPUT).unwrap();
        info!(
            "MonographInstall install database  output = {:?}",
            TaskValue::into_inner_value::<String>(install_db_output.clone())
        );
        if 0 != status_code {
            error!(
                "MonographInstall install_db={} execution failed status_code={}",
                install_db_script, status_code
            );
            Err(anyhow!(CmdErr::MonographInstallErr(
                install_db_script,
                status_code.to_string()
            )))
        } else {
            Ok(Some(HashMap::from([(
                SOURCE_PATH.to_string(),
                TaskValue::Str(format!("{}/datafarm", remote_install_dir)),
            )])))
        }
    }
}
