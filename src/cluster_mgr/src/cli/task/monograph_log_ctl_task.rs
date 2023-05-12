use crate::cli::ssh::SSHSession;
use crate::cli::task::task_base::{
    ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId, TaskInstance,
};
use crate::cli::task::task_utils::{check_pid, parse_process_pid_as_list};
use crate::cli::CommandArgs;
use crate::config::config_base::DeploymentConfig;
use crate::config::deployment::LogProcessKey;
use crate::get_ctl_cmd_string;
// use futures::future;
use indexmap::IndexMap;
use itertools::Itertools;
use std::collections::HashMap;
// use tracing::info;

const CLUSTER_COMMAND_STR: &str = "cluster_cmd";
const FIND_LOG_PROCESS_CMD: &str = r#"ps uxwe -u _USER | grep -E 'conf=_MEMBER_CONF|_LOG_BIN_CMD' \
| grep -v grep | awk '{print _COLUMN}'"#;

// const AWK_PRINT_PID: &str =
//     r#"awk '{printf "%s", sep $0; sep = "_SEP"}; END {if (NR) print ""}'"#;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LogCtlCmd {
    Start(String),
    Stop(String),
    Status(String),
}

get_ctl_cmd_string!(LogCtlCmd, Start, Stop, Status);

impl LogCtlCmd {
    pub fn build_cmd(
        config: &DeploymentConfig,
        cmd_arg: CommandArgs,
    ) -> HashMap<LogProcessKey, LogCtlCmd> {
        LogCtlCmd::build_cmd_with_predicate(
            config,
            cmd_arg,
            None::<Box<dyn Fn(String, u16) -> bool>>,
        )
    }

    fn build_cmd_with_predicate<F>(
        config: &DeploymentConfig,
        cmd_arg: CommandArgs,
        test: Option<Box<F>>,
    ) -> HashMap<LogProcessKey, LogCtlCmd>
    where
        F: Fn(String, u16) -> bool + ?Sized,
    {
        let log_home_dir_binding = config.log_home_dir();
        let log_home = log_home_dir_binding.as_str();
        let user = &config.connection.username;
        let log_srv = config.deployment.log_service.as_ref().unwrap();
        let log_cmd_binding = log_srv.log_start_cmd();
        let log_cmds = log_cmd_binding.values().flatten().collect_vec();

        log_cmds
            .iter()
            .filter(|log_item| {
                let log_port = log_item.log_member.port;
                let host = &log_item.log_member.member_host;
                if let Some(predicate) = &test {
                    predicate(host.clone(), log_port)
                } else {
                    true
                }
            })
            .map(|cmd_items| {
                let log_port = cmd_items.log_member.port;
                let host = &cmd_items.log_member.member_host;

                let ps_cmd_part = FIND_LOG_PROCESS_CMD
                    .replace("_USER", user)
                    .replace("_MEMBER_CONF", cmd_items.group_members_config.as_str())
                    .replace("_LOG_BIN_CMD", format!("{log_home}/bin/launch_sv").as_str())
                    .replace("_COLUMN", "$2");
                let log_cmd = match &cmd_arg {
                    CommandArgs::Start { cluster: _ } => LogCtlCmd::Start(format!(
                        "{log_home}/start_log_{}.bash",
                        format_args!("{host}_{log_port}")
                    )),
                    CommandArgs::Status {
                        cluster: _,
                        user: _,
                        password: _,
                    } => LogCtlCmd::Status(ps_cmd_part),
                    CommandArgs::Stop { cluster: _, force } => {
                        let ps_log_info = ps_cmd_part;
                        let is_force = force.is_some();
                        let stop_cmd_string = if is_force {
                            format!("{ps_log_info} | xargs kill -9")
                        } else {
                            format!("{ps_log_info} | xargs kill")
                        };
                        LogCtlCmd::Stop(stop_cmd_string)
                    }
                    _ => unreachable!(),
                };
                let process_key = LogProcessKey {
                    host: host.clone(),
                    port: log_port,
                };
                (process_key.clone(), log_cmd)
            })
            .collect::<HashMap<LogProcessKey, LogCtlCmd>>()
    }
}

#[derive(Clone, Debug)]
pub struct MonographLogCtlTask {
    config: DeploymentConfig,
    task_id: TaskId,
    log_cmd: HashMap<LogProcessKey, LogCtlCmd>,
}

impl MonographLogCtlTask {
    pub fn new(
        config: DeploymentConfig,
        task_id: TaskId,
        log_cmd: HashMap<LogProcessKey, LogCtlCmd>,
    ) -> Self {
        Self {
            config,
            task_id,
            log_cmd,
        }
    }

    pub fn from_config(
        cmd_arg: CommandArgs,
        config: &DeploymentConfig,
    ) -> IndexMap<TaskId, TaskInstance> {
        let log_cmd_by_key = LogCtlCmd::build_cmd(config, cmd_arg.clone());
        let user = &config.connection.username;
        let port = config.connection.ssh_port() as usize;

        let cluster_arg_ref = cmd_arg.as_ref();
        log_cmd_by_key
            .iter()
            .into_group_map_by(|(process_key, _cmd)| process_key.host.clone())
            .into_iter()
            .map(|(host, key_cmd_pair)| {
                let task_host = TaskHost::Remote {
                    user: user.to_string(),
                    port,
                    hosts: host.to_string(),
                };
                let task_id = TaskId {
                    cmd: "monograph_log_ctl".to_string(),
                    task: cmd_arg.as_ref().to_string(),
                    host,
                };

                let log_cmd = key_cmd_pair
                    .iter()
                    .map(|pair| (pair.0.clone(), pair.1.clone()))
                    .collect::<HashMap<LogProcessKey, LogCtlCmd>>();

                (
                    task_id.clone(),
                    TaskInstance {
                        task_input: HashMap::from([(
                            CLUSTER_COMMAND_STR.to_string(),
                            TaskArgValue::Str(cluster_arg_ref.to_string()),
                        )]),
                        task: Box::new(MonographLogCtlTask::new(config.clone(), task_id, log_cmd)),
                        task_host,
                    },
                )
            })
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }

    async fn log_pid(
        &self,
        ssh_session: &SSHSession,
    ) -> anyhow::Result<HashMap<LogProcessKey, ExecutionValue>> {
        let cluster_status_cmd = CommandArgs::Status {
            cluster: self.config.deployment.cluster_name.to_string(),
            user: None,
            password: None,
        };
        let check_status_cmd = self
            .log_cmd
            .iter()
            .flat_map(|(process_key, _log_cmd)| {
                LogCtlCmd::build_cmd_with_predicate(
                    &self.config,
                    cluster_status_cmd.clone(),
                    Some(Box::new(|host: String, port| -> bool {
                        process_key.host.eq(host.as_str()) && port == process_key.port
                    })),
                )
            })
            .collect::<HashMap<LogProcessKey, LogCtlCmd>>();

        let find_log_process_cmd = MonographLogCtlTask::remote_cmd(check_status_cmd);
        let result = check_pid(
            find_log_process_cmd,
            ssh_session.clone(),
            parse_process_pid_as_list,
        )
        .await?;
        Ok(HashMap::new())
        // info!("LogCtlTask check_process_status={cluster_status_cmd:?}");
        // let cmd_result = check_status_cmd
        //     .iter()
        //     .map(|(key, ctl_cmd)| (key, ctl_cmd.cmd_value()))
        //     .map(|(key, cmd_string)| async move {
        //         let cmd_rs =
        //             check_process_pid(cmd_string.clone(), ssh_session.clone(), parse_process_pid)
        //                 .await;
        //         (key.clone(), cmd_rs)
        //     })
        //     .collect_vec();
        // let all_result = future::join_all(cmd_result).await;
        // all_result
        //     .iter()
        //     .filter(|(_, rs)| rs.is_ok())
        //     .map(|(key, rs)| {
        //         let value = rs.as_ref().unwrap();
        //         (key.clone(), value.clone())
        //     })
        //     .collect::<HashMap<LogProcessKey, ExecutionValue>>()
    }

    fn remote_cmd(log_ctl_cmd: HashMap<LogProcessKey, LogCtlCmd>) -> String {
        log_ctl_cmd
            .iter()
            .map(|(_, ctl_cmd)| ctl_cmd.cmd_value())
            .join(";")
    }
}

#[async_trait::async_trait]
impl TaskExecutor for MonographLogCtlTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        task_arg: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        let cluster_mgr_cmd = task_arg.get(CLUSTER_COMMAND_STR).unwrap();
        let cluster_cmd_string = cluster_mgr_cmd.clone().into_inner_value::<String>();
        println!("{} execute.\n", self.task_id.pretty_string());
        let ssh_session =
            SSHSession::from_task_host(task_host, self.config.connection.ssh_auth_key().unwrap())
                .await?;
        let pid_info = self.log_pid(&ssh_session).await?;
        // if cluster_cmd_string.eq("status") {
        //     let status_result = pid_info
        //         .iter()
        //         .flat_map(|(key, value)| {
        //             let log_process = key.to_string();
        //             value
        //                 .iter()
        //                 .map(|(key, value)| (key, value.clone()))
        //                 .collect::<ExecutionValue>()
        //         })
        //         .collect::<ExecutionValue>();
        //     ssh_session.close().await?;
        //     task_return_value!(
        //         status_result,
        //         |status_code: usize| -> CmdErr {
        //             CmdErr::ExecUserCmdErr(cluster_cmd_string.clone(), status_code.to_string())
        //         },
        //         "MonographLogCtlTask"
        //     )
        // } else {
        //     ssh_session.close().await?;
        //     Ok(None)
        // }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::task::task_base::{ExecutionValue, TaskArgValue};
    use crate::cli::{CMD, CMD_STATUS};
    use itertools::Itertools;
    use std::collections::HashMap;

    #[test]
    pub fn test_merge_execution_value() {
        let val1 = HashMap::from([
            (CMD.to_string(), TaskArgValue::Str("cmd1".to_string())),
            (CMD_STATUS.to_string(), TaskArgValue::Number(0)),
        ]);

        let val2 = HashMap::from([
            (CMD.to_string(), TaskArgValue::Str("cmd2".to_string())),
            (CMD_STATUS.to_string(), TaskArgValue::Number(2)),
        ]);

        val1.iter().for_each(|(key, val)| {

        });
        //println!("{merges_vals:#?}")
    }
}
