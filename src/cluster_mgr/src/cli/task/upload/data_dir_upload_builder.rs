use crate::cli::task::task_base::{TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::task::upload::upload_task::UploadTask;
use crate::cli::task::upload::upload_task_builder::UploadTaskBuilder;
use crate::cli::task::upload::{COPY_DIR, SOURCE_HOST, SOURCE_PATH};
use crate::config::config_base::DeploymentConfig;
use crate::config::DeploymentPackage;
use indexmap::IndexMap;
use itertools::Itertools;
use std::collections::HashMap;

pub struct DataDirUploadBuilder;

impl UploadTaskBuilder for DataDirUploadBuilder {
    /// Upload the MonographDB data_dir to the remote host.
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance> {
        if config.get_host_list(DeploymentPackage::MonographTx).len() == 1 {
            return IndexMap::new();
        }
        let deployment_ref = &config.deployment;
        let bootstrap_host = deployment_ref.bootstrap_host();
        let ssh_conn_ref = &config.connection;
        let conn_user = &ssh_conn_ref.username;
        let ssh_port = ssh_conn_ref.ssh_port() as usize;

        let dest_hosts = deployment_ref
            .tx_service
            .host
            .iter()
            .filter(|host| !host.as_str().eq(bootstrap_host.as_str()))
            .cloned()
            .collect_vec();

        let datafarm = format!("{}/datafarm", config.install_dir());
        dest_hosts
            .iter()
            .map(|dest_host| {
                let task_id = TaskId {
                    cmd: "install".to_string(),
                    task: "upload_datafarm".to_string(),
                    host: dest_host.clone(),
                };
                (
                    task_id.clone(),
                    TaskInstance {
                        task_input: HashMap::from([
                            (SOURCE_PATH.to_string(), TaskArgValue::Str(datafarm.clone())),
                            (COPY_DIR.to_string(), TaskArgValue::Str("-r".to_string())),
                            (
                                SOURCE_HOST.to_string(),
                                TaskArgValue::Str(bootstrap_host.clone()),
                            ),
                        ]),
                        task: Box::new(UploadTask::new(config.clone(), task_id)),
                        task_host: TaskHost::Remote {
                            user: conn_user.to_string(),
                            port: ssh_port,
                            hosts: dest_host.to_string(),
                        },
                    },
                )
            })
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }
}
