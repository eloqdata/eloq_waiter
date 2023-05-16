use crate::cli::task::group::{LogServiceCtlTaskGroup, TaskGroup};
use crate::cli::task::monograph_log_ctl_task::MonographLogCtlTask;
use crate::cli::task::monograph_log_probe_task::MonographLogProbeTask;
use crate::cli::task::task_base::{TaskExecutionContext, TaskId, TaskInstance};
use crate::cli::CommandArgs;
use crate::config::config_base::DeploymentConfig;
use indexmap::IndexMap;

impl LogServiceCtlTaskGroup {
    pub(crate) fn log_ctl_tasks(
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> IndexMap<TaskId, TaskInstance> {
        let mut log_ctl_executable = IndexMap::new();
        log_ctl_executable.extend(MonographLogCtlTask::from_config(cmd_arg, &config).into_iter());
        log_ctl_executable.extend(MonographLogProbeTask::from_config(&config).into_iter());
        log_ctl_executable
    }
}

#[async_trait::async_trait]
impl TaskGroup for LogServiceCtlTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let log_ctl_cmd_name = match cmd_arg.clone() {
            CommandArgs::LogService {
                cluster: _,
                command: log_ctl_cmd,
            } => log_ctl_cmd,
            _ => unreachable!(),
        };

        let log_ctl_executable = LogServiceCtlTaskGroup::log_ctl_tasks(cmd_arg, config);
        Ok(TaskExecutionContext {
            task_group: format!("log-srv-{log_ctl_cmd_name}"),
            barrier: None,
            executable: log_ctl_executable,
        })
    }
}
