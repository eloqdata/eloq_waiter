use crate::cli::config::DeploymentConfig;
use crate::cli::task::ssh_conn::{SSH_EXEC_CMD_OUTPUT, SSH_EXEC_CMD_STATUS};
use crate::cli::task::task_base::{
    CmdErr, ExecutionResult, TaskExecutionContext, TaskExecutor, TaskHost, TaskId, TaskValue,
};
use crate::ssh_conn_info;
use anyhow::anyhow;
use async_trait::async_trait;
use itertools::Itertools;
use std::collections::HashMap;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct ExecCustomCommand {
    cmd: String,
    task_id: TaskId,
    config: DeploymentConfig,
}

impl ExecCustomCommand {
    pub fn from_config(cmd_string: String, config: &DeploymentConfig) -> Vec<TaskExecutionContext> {
        let all_hosts = config.get_host_as_map();
        let conn_user = &config.connection.username;
        let ssh_port = config.connection.ssh_port();
        all_hosts
            .values()
            .flat_map(|hosts| {
                hosts
                    .iter()
                    .map(|host_val| {
                        let task_host = TaskHost::Remote {
                            user: conn_user.clone(),
                            port: ssh_port as usize,
                            hosts: host_val.clone(),
                        };
                        TaskExecutionContext {
                            task_input: HashMap::default(),
                            task: Box::new(ExecCustomCommand::new(
                                cmd_string.clone(),
                                TaskId {
                                    cmd: "exec_cmd".to_string(),
                                    task: "".to_string(),
                                },
                                config.clone(),
                            )),
                            task_host,
                        }
                    })
                    .collect_vec()
            })
            .collect_vec()
    }

    pub fn new(cmd: String, task_id: TaskId, config: DeploymentConfig) -> Self {
        Self {
            cmd,
            task_id,
            config,
        }
    }
}

#[async_trait]
impl TaskExecutor for ExecCustomCommand {
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
            ssh_conn_rs,
            _conn_user,
            conn_host
        }

        let ssh_conn = ssh_conn_rs?;
        let exec_cmd_rs = ssh_conn.run_cmd_sync_output(self.cmd.clone())?;
        let status_code_value = exec_cmd_rs.get(SSH_EXEC_CMD_STATUS).unwrap();
        let status_code = TaskValue::into_inner_value::<usize>(status_code_value.clone());
        if status_code != 0 {
            error!(
                "ExecCustomCommand execute remote cmd={} error, status_code={}, host={}",
                self.cmd, status_code, conn_host
            );
            Err(anyhow!(CmdErr::ExecUserCmdErr(self.cmd.clone())))
        } else {
            let cmd_output = exec_cmd_rs.get(SSH_EXEC_CMD_OUTPUT).unwrap();
            let output_string = TaskValue::into_inner_value::<String>(cmd_output.clone());
            info!(
                "ExecCustomCommand remote command={} on remote host={} execute success",
                self.cmd, conn_host
            );
            println!(
                r#"Host {}, Command output :
                    {}
             "#,
                conn_host, output_string
            );
            Ok(None)
        }
    }
}
