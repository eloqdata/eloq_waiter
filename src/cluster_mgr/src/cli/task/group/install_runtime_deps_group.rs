use crate::cli::task::group::{InstallRuntimeDepsTaskGroup, TaskGroup};
use crate::cli::task::runtime_deps_install::RuntimeDepsInstallation;
use crate::cli::task::task_base::TaskExecutionContext;
use crate::cli::CommandArgs;
use crate::config::config_base::DeploymentConfig;

#[async_trait::async_trait]
impl TaskGroup for InstallRuntimeDepsTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let install_runtime_deps = RuntimeDepsInstallation::from_config(&config)?;
        Ok(TaskExecutionContext {
            task_group: cmd_arg.as_ref().to_string(),
            barrier: None,
            executable: install_runtime_deps,
        })
    }
}
