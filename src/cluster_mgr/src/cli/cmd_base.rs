use crate::cli::config::DeploymentConfig;
use crate::cli::task::task_base::TaskMgr;
use crate::cli::CommandArgs;
use crate::state::deployment_operation::{DeploymentEntity, DeploymentOperation};
use crate::state::state_base::{QueryCondition, StateOperation};
use crate::state::state_mgr::{StateMgr, DEPLOYMENT_STATE, STATE_MGR};
use crate::StateValue;
use itertools::Itertools;
use tracing::info;

#[derive(Clone)]
pub struct CommandExecutor {
    task_mgr: TaskMgr,
    state_mgr: StateMgr,
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandExecutor {
    pub fn new() -> Self {
        Self {
            task_mgr: TaskMgr::default(),
            state_mgr: STATE_MGR.clone(),
        }
    }

    pub async fn get_config(&self, cmd: CommandArgs) -> anyhow::Result<DeploymentConfig> {
        match cmd.clone() {
            CommandArgs::Deploy { topology_file } => {
                let config_rs = DeploymentConfig::load(Some(topology_file));
                let config = config_rs.unwrap().clone();
                let deployment_operation = self
                    .state_mgr
                    .get_state_operation::<DeploymentOperation>(DEPLOYMENT_STATE);

                let all_hosts = config
                    .get_host_as_map()
                    .iter()
                    .flat_map(|entry| entry.1)
                    .cloned()
                    .collect_vec()
                    .join(";");

                let config_string = config.config_string();
                info!(
                    "CmdExecutor save DeploymentConfig {} {}",
                    config_string, all_hosts
                );
                deployment_operation
                    .put(DeploymentEntity {
                        cluster_name: config.deployment.clone().cluster_name,
                        deployment_config: config_string,
                        host_list: all_hosts,
                        create_timestamp: Default::default(),
                        update_timestamp: Default::default(),
                    })
                    .await?;
                info!("CmdExecutor Save DeploymentConfig successfully.");
                Ok(config)
            }
            CommandArgs::Install { cluster } => {
                let deployment_operation = self
                    .state_mgr
                    .get_state_operation::<DeploymentOperation>(DEPLOYMENT_STATE);

                let entity = deployment_operation
                    .load(|| -> Option<QueryCondition> {
                        Some(QueryCondition {
                            cond_text: " cluster_name=$1".to_string(),
                            bind_values: vec![StateValue::Varchar(cluster.clone())],
                        })
                    })
                    .await?;
                assert_eq!(entity.len(), 1);
                let deployment_entity = entity.first().unwrap();
                let config_content = deployment_entity.clone().deployment_config;
                DeploymentConfig::load_from_string(config_content)
            }
            _ => {
                unimplemented!()
            }
        }
    }

    pub async fn run(&'static self, cmd: CommandArgs) -> anyhow::Result<()> {
        let config = self.get_config(cmd.clone()).await?;
        info!(
            "CmdExecutor load config from StateMgr successfully.{:#?}",
            config
        );
        let join = tokio::task::spawn(async move {
            self.task_mgr.receive_task_result().await;
        });
        self.task_mgr.build_and_run(cmd.clone(), config).await?;
        join.await?;
        Ok(())
    }
}
