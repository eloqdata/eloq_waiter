use crate::cli::task::group::{
    CtrlDBTaskGroup, DeploymentTaskGroup, InstallDBTaskGroup, InstallRuntimeDepsTaskGroup,
    MonitorCtlTaskGroup, PlayTaskGroup, TaskGroup,
};
use crate::cli::task::task_base::{merge_execution, TaskExecutionContext};
use crate::cli::CommandArgs;
use crate::config::config_base::DeploymentConfig;

#[async_trait::async_trait]
impl TaskGroup for PlayTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let topo_file = match cmd_arg.clone() {
            CommandArgs::Play { topology_file } => topology_file,
            _ => {
                unreachable!()
            }
        };
        let (barrier, executable) = merge_execution(vec![
            InstallRuntimeDepsTaskGroup
                .tasks(
                    CommandArgs::RunDeps {
                        topology_file: topo_file.clone(),
                    },
                    config.clone(),
                )
                .await?,
            DeploymentTaskGroup
                .tasks(
                    CommandArgs::Deploy {
                        topology_file: topo_file.clone(),
                    },
                    config.clone(),
                )
                .await?,
            InstallDBTaskGroup
                .tasks(
                    CommandArgs::Install {
                        cluster: config.deployment.cluster_name.clone(),
                    },
                    config.clone(),
                )
                .await?,
            CtrlDBTaskGroup
                .tasks(
                    CommandArgs::Start {
                        cluster: config.deployment.cluster_name.clone(),
                    },
                    config.clone(),
                )
                .await?,
            MonitorCtlTaskGroup
                .tasks(
                    CommandArgs::Monitor {
                        cluster: config.deployment.cluster_name.clone(),
                        command: "start".to_string(),
                    },
                    config.clone(),
                )
                .await?,
        ]);

        Ok(TaskExecutionContext {
            task_group: cmd_arg.as_ref().to_string(),
            barrier: Some(barrier),
            executable,
        })
    }
}
