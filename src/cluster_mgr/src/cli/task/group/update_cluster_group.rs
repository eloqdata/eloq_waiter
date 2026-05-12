use crate::cli::task::group::{Config, TaskGroup, UpdateClusterTaskGroup};
use crate::cli::task::rolling_upgrade::{self, steps::UpgradeContext};
use crate::cli::task::task_base::TaskExecutionContext;
use crate::cli::SubCommand;

#[async_trait::async_trait]
impl TaskGroup for UpdateClusterTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: SubCommand,
        config: &Config,
    ) -> anyhow::Result<TaskExecutionContext> {
        let update_eloq = matches!(
            &cmd_arg,
            SubCommand::Update {
                version: Some(_),
                ..
            }
        );
        if !update_eloq {
            return Ok(TaskExecutionContext::dummy());
        }

        use rolling_upgrade::{steps, RollingUpgrade};
        let ctx = UpgradeContext::new(&cmd_arg, config.clone());
        let steps = steps::build_upgrade_steps(ctx);
        let ru = RollingUpgrade::new(steps, config.clone());
        ru.execute().await?;

        Ok(TaskExecutionContext::dummy())
    }
}
