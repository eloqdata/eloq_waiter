use crate::cli::task::group::{InstallDepPkgTaskGroup, TaskGroup};
use crate::cli::task::install_dep_pkg::DepPkgTask;
use crate::cli::task::task_base::TaskExecutionContext;
use crate::cli::SubCommand;
use crate::config::config_base::DeployConfig;

#[async_trait::async_trait]
impl TaskGroup for InstallDepPkgTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: SubCommand,
        config: DeployConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let install_runtime_deps = DepPkgTask::from_config(&config)?;
        Ok(TaskExecutionContext {
            task_group: cmd_arg.as_ref().to_string(),
            barrier: None,
            executable: install_runtime_deps,
        })
    }
}
