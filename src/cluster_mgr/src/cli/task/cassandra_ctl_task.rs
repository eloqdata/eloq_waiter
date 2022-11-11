use crate::cli::config::{DeploymentConfig, DeploymentService};
use crate::cli::task::ssh_conn::{SSHConn, SSH_EXEC_CMD_OUTPUT, SSH_EXEC_CMD_STATUS};
use crate::cli::task::task_base::{
    CmdErr::CassandraCtlErr, ExecutionResult, TaskExecutionContext, TaskExecutor, TaskHost, TaskId,
    TaskValue,
};
use crate::cli::CommandArgs;
use crate::ssh_conn_info;
use anyhow::anyhow;
use async_trait::async_trait;
use itertools::Itertools;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use strum_macros::AsRefStr;
use tracing::{error, info};

pub(crate) const CASSANDRA_CMD_STR: &str = "cassandra_cmd";

#[macro_export]
macro_rules! cassandra_cmd {
    ( $cmd:ty, $cassandra_home:expr $(, $cmd_arg:expr)? $(,)?) => {{
        use $crate::cli::task::task_base::REMOTE_ENV_PROPS;
        let cmd_var = stringify!($cmd);
        let remote_env_props = REMOTE_ENV_PROPS.as_ref().unwrap();
        let java_home = remote_env_props.get("JAVA_HOME").unwrap();
        match cmd_var {
            "CassandraCmd::Start" => CassandraCmd::Start(format!(
                r#"mkdir -p {}/logs && cd {} && export JAVA_HOME={}; {}/bin/cassandra -f > {}/logs/cassandra_start.log 2>&1 &"#,
                //r#"bash {}/start_cassandra.bash"#,
                $cassandra_home, $cassandra_home, java_home, $cassandra_home, $cassandra_home
            )),
            "CassandraCmd::Status" => CassandraCmd::Status(format!("export JAVA_HOME={}; {}/bin/nodetool status", java_home, $cassandra_home)),
            $("CassandraCmd::Stop" => {
                let pid = $cmd_arg;
                CassandraCmd::Stop(format!("kill {}", pid))
            },
            "CassandraCmd::ProcessInfo" => {
                let cmd_user = $cmd_arg;
                let echo_cmd=format!(" echo `ps -o pid,ruser={},command | grep cassandra | grep -v grep", cmd_user);
                let print_pid = r#"| awk '{print $1}'`"#;
                let pid_cmd = format!("{} {}", echo_cmd, print_pid);
                let pid_cwd = r#"{ read pid; cmd="readlink /proc/$pid/cwd"; output=`eval $cmd`; echo "$pid:$output"}"#;
                let final_cmd = format!("{} | {}", pid_cmd, pid_cwd);
                CassandraCmd::ProcessInfo(final_cmd)
            },)*
            _=> {
               unreachable!()
            }
        }
    }};
}

#[macro_export]
macro_rules! cassandra_ctl {
    ($task_host:expr,$cmd:expr, $cmd_var:ident, $ssh_conn:expr, $self:ident, $check_fn:ident) => {{
        let cmd_rs = match $cmd.clone() {
            CassandraCmd::$cmd_var(_cmd) => {
                let running_rs = $self.already_running($ssh_conn.clone(), $task_host);
                if let Ok(pid_opt) = running_rs {
                    if pid_opt.$check_fn() {
                        $self.execute_cassandra_cmd($ssh_conn, $cmd.clone())
                    } else {
                        Ok(true)
                    }
                } else {
                    Err(anyhow!(running_rs.err().unwrap().to_string()))
                }
            }
            _ => {
                unreachable!()
            }
        };
        cmd_rs
    }};
}

#[derive(Clone, Debug, Eq, PartialEq, AsRefStr)]
pub enum CassandraCmd {
    #[strum(serialize = "Start")]
    Start(String),
    #[strum(serialize = "Stop")]
    Stop(String),
    #[strum(serialize = "Status")]
    Status(String),
    #[strum(serialize = "ProcessInfo")]
    ProcessInfo(String),
}

impl CassandraCmd {
    pub fn from_string(cmd_str: String, cassandra_home: String, conn_user: Option<String>) -> Self {
        match cmd_str.to_lowercase().as_str() {
            "start" => {
                cassandra_cmd!(CassandraCmd::Start, cassandra_home)
            }
            "stop" => {
                cassandra_cmd!(CassandraCmd::Stop, cassandra_home)
            }
            "status" => {
                cassandra_cmd!(CassandraCmd::Status, cassandra_home)
            }
            "processinfo" => {
                let user = conn_user.unwrap();
                cassandra_cmd!(CassandraCmd::ProcessInfo, cassandra_home, user)
            }
            _ => {
                unreachable!()
            }
        }
    }

    pub fn cmd_value(&self) -> String {
        match self.clone() {
            CassandraCmd::ProcessInfo(cmd)
            | CassandraCmd::Start(cmd)
            | CassandraCmd::Stop(cmd)
            | CassandraCmd::Status(cmd) => cmd,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CassandraCtlTask {
    config: DeploymentConfig,
    task_id: TaskId,
}

impl CassandraCtlTask {
    pub fn from_config(cmd: CommandArgs, config: &DeploymentConfig) -> Vec<TaskExecutionContext> {
        let cassandra_task_ctrl_attr = match cmd {
            CommandArgs::Start { cluster: _ } | CommandArgs::Install { cluster: _ } => (
                "start",
                TaskId {
                    cmd: "start".to_string(),
                    task: "cassandra-start".to_string(),
                },
            ),
            CommandArgs::Stop { cluster: _ } => (
                "stop",
                TaskId {
                    cmd: "stop".to_string(),
                    task: "cassandra-stop".to_string(),
                },
            ),
            _ => {
                unreachable!()
            }
        };
        let cmd_str = cassandra_task_ctrl_attr.0;
        let task_id = cassandra_task_ctrl_attr.1;
        let conn_user = config.connection.clone().username;
        let ssh_port = config.connection.ssh_port();
        let cassandra_hosts = config.get_host_list(DeploymentService::Storage);
        cassandra_hosts
            .iter()
            .map(|host| TaskExecutionContext {
                task_input: HashMap::from([(
                    CASSANDRA_CMD_STR.to_string(),
                    TaskValue::Str(cmd_str.to_string()),
                )]),
                task: Box::new(CassandraCtlTask {
                    config: config.clone(),
                    task_id: task_id.clone(),
                }),
                task_host: TaskHost::Remote {
                    user: conn_user.clone(),
                    port: ssh_port as usize,
                    hosts: host.clone(),
                },
            })
            .collect_vec()
    }

    pub fn new(config: DeploymentConfig, task_id: TaskId) -> Self {
        Self { config, task_id }
    }

    fn cassandra_home(&self) -> String {
        format!("{}/apache-cassandra", self.config.install_dir())
    }

    fn already_running(
        &self,
        ssh_conn: SSHConn,
        task_host: TaskHost,
    ) -> anyhow::Result<Option<i32>> {
        let conn_user = task_host.ssh_conn_tuple().0;
        let cassandra_home = self.cassandra_home();
        let cassandra_process =
            cassandra_cmd!(CassandraCmd::ProcessInfo, cassandra_home, conn_user);
        let process_info = cassandra_process.cmd_value();
        let cmd_exec_rs = ssh_conn.run_cmd(process_info.clone(), true)?;
        let cmd_status = cmd_exec_rs.get(SSH_EXEC_CMD_STATUS).unwrap();

        if 0 != TaskValue::into_inner_value::<usize>(cmd_status.clone()) {
            error!(
                "CassandraCtlTask CassandraCmd::ProcessInfo fails status={:?}",
                cmd_status
            );
            return Err(anyhow!("Cmd {} execution fails", process_info));
        }
        let cmd_output_value = cmd_exec_rs.get(SSH_EXEC_CMD_OUTPUT).unwrap();

        let output = TaskValue::into_inner_value::<String>(cmd_output_value.clone());
        info!(
            "CassandraCtlTask CassandraCmd::ProcessInfo cmd={},output={}",
            process_info, output
        );
        let mut pid = None;
        for line in output.lines() {
            let splits = line.split(':').collect_vec();
            if splits.is_empty() || splits.len() == 1 {
                continue;
            }
            assert_eq!(splits.len(), 2);
            let process_cmd = splits[1];
            if cassandra_home.as_str() == process_cmd {
                let pid_num = splits[0].parse::<i32>().unwrap();
                pid = Some(pid_num);
                info!(
                    "CassandraCtlTask found cassandra process is already running PID={}",
                    pid_num
                );
                break;
            }
        }
        Ok(pid)
    }

    fn wait_cassandra_start_complete(
        &self,
        wait_timeout: Duration,
        ssh_conn: &SSHConn,
    ) -> anyhow::Result<bool> {
        let cassandra_home = self.cassandra_home();
        let check_status = cassandra_cmd!(CassandraCmd::Status, cassandra_home);
        let check_status_cmd = check_status.cmd_value();
        info!("CassandraCtlTask CheckStatus cmd={:?}", check_status_cmd);
        let sleep_duration = Duration::from_secs(1);
        let mut timeout_remaining = wait_timeout;
        let mut process_ready = false;
        loop {
            if timeout_remaining.as_secs() == 0 {
                info!("CassandraCtlTask CheckStatus timeout");
                break;
            }
            let rs = ssh_conn.run_cmd(check_status_cmd.clone(), true);
            if rs.as_ref().is_err() {
                let check_status_cmd_err = rs.err().unwrap().to_string();
                error!(
                    "CassandraCtlTask CheckStatus return failed. {}",
                    check_status_cmd_err
                );
                return Err(anyhow!(CassandraCtlErr(
                    check_status_cmd,
                    check_status_cmd_err
                )));
            }
            let exec_rs = rs.as_ref().unwrap();
            let output_value = exec_rs.get(SSH_EXEC_CMD_OUTPUT).unwrap();
            let output_string = TaskValue::into_inner_value::<String>(output_value.clone());
            info!("CassandraCtlTask CheckStatus output={}", output_string);
            for line in output_string.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if !line.contains(char::is_whitespace) {
                    continue;
                }
                info!("CassandraCtlTask  status={}", line);
                if line.starts_with("UN") {
                    process_ready = true;
                }
            }
            if process_ready {
                info!("CassandraCtlTask CheckStatus found cassandra process ready.");
                break;
            }
            thread::sleep(sleep_duration);
            timeout_remaining -= sleep_duration;
        }
        info!(
            "CassandraCtlTask found  Cassandra process {}",
            process_ready
        );
        Ok(process_ready)
    }

    fn cassandra_start(&self, start_cmd: String, ssh_conn: &SSHConn) -> anyhow::Result<bool> {
        let start_status = ssh_conn.run_cmd(start_cmd.clone(), true)?;
        let status_code = TaskValue::into_inner_value::<usize>(
            start_status.get(SSH_EXEC_CMD_STATUS).unwrap().clone(),
        );
        info!(
            "CassandraCtlTask CassandraCmd::Start cmd={},status_code={}",
            start_cmd, status_code
        );
        if status_code != 0 {
            return Err(anyhow!(format!(
                "Start cassandra fails cmd={}, cassandra_home={},cmd_code={}",
                start_cmd,
                self.cassandra_home(),
                status_code
            )));
        }
        self.wait_cassandra_start_complete(Duration::from_secs(60 * 5), ssh_conn)
    }

    fn cassandra_stop(&self, stop_cmd: String, ssh_conn: &SSHConn) -> anyhow::Result<bool> {
        let stop_status = ssh_conn.run_cmd(stop_cmd.clone(), false)?;
        info!(
            "CassandraCtlTask CassandraCmd::Stop cmd={},status_code={:?}",
            stop_cmd, stop_status,
        );
        let stop_cmd_success = TaskValue::into_inner_value::<usize>(
            stop_status.get(SSH_EXEC_CMD_STATUS).unwrap().clone(),
        ) == 0;
        Ok(stop_cmd_success)
    }

    pub fn execute_cassandra_cmd(
        &self,
        ssh_conn: SSHConn,
        cmd: CassandraCmd,
    ) -> anyhow::Result<bool> {
        let ctl_rsp = match cmd {
            CassandraCmd::Stop(stop_cmd) => self.cassandra_stop(stop_cmd, &ssh_conn)?,
            CassandraCmd::Start(start_cmd) => self.cassandra_start(start_cmd, &ssh_conn)?,
            _ => {
                unreachable!()
            }
        };
        Ok(ctl_rsp)
    }
}

#[async_trait]
impl TaskExecutor for CassandraCtlTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        task_arg: HashMap<String, TaskValue>,
    ) -> anyhow::Result<Option<ExecutionResult>> {
        ssh_conn_info! {
            self.config.connection.clone(),
            task_host.clone(),
            ssh_conn_rs,
            _conn_user,
            _conn_host
        }
        let cmd_str =
            TaskValue::into_inner_value::<String>(task_arg.get(CASSANDRA_CMD_STR).unwrap().clone());
        let cassandra_home = self.cassandra_home();

        info!(
            "CassandraCtlTask will be run. Cmd={:?}, cassandra_home={:?}",
            cmd_str, cassandra_home
        );
        let cmd = CassandraCmd::from_string(cmd_str, cassandra_home, None);
        let ssh_conn = ssh_conn_rs?;
        let exec_rs = if cmd.as_ref() == "Start" {
            cassandra_ctl!(task_host, cmd, Start, ssh_conn, self, is_none)
        } else if cmd.as_ref() == "Stop" {
            cassandra_ctl!(task_host, cmd, Stop, ssh_conn, self, is_some)
        } else {
            unreachable!()
        };
        if let Err(err) = exec_rs {
            Err(anyhow!(err.to_string()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::task::cassandra_ctl_task::CassandraCmd;

    #[test]
    pub fn test_build_cassandra_cmd() {
        let cassandra_bin = "/data1/opt/mono-poc";
        let cassandra_process = cassandra_cmd!(CassandraCmd::ProcessInfo, cassandra_bin, "mono");
        println!("start = {:#?}", cassandra_process);
    }

    #[test]
    pub fn test_string_compare() {
        let str_value = ":".to_string();
        println!("eq={}", str_value.is_empty() || str_value == ":")
    }
}
