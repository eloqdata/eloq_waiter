use crate::cli::task::task_base::{TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::task::upload::upload_task::UploadTask;
use crate::cli::task::upload::upload_task_builder::UploadTaskBuilder;
use crate::cli::task::upload::{DEST_PATH, SOURCE_PATH};
use crate::config::config_base::DeploymentConfig;
use indexmap::IndexMap;
use std::collections::HashMap;

pub struct CassConfUploadBuilder;

impl UploadTaskBuilder for CassConfUploadBuilder {
    /// Upload the cassandra.yaml and jvm11-server.options or cassandra-env.sh file to the remote host (remote host list from deployment.yaml).
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance> {
        let ssh_port = config.connection.ssh_port();
        let conn_user = config.clone().connection.username;
        let deployment = config.deployment.clone();
        let monitor = deployment.monitor;
        let install_dir = config.install_dir();

        let cass_config_rs = config.deployment.storage_service.gen_cassandra_config(
            install_dir,
            deployment.cluster_name,
            monitor,
        );
        assert!(cass_config_rs.is_ok());
        let cass_config = cass_config_rs.unwrap();
        cass_config
            .into_iter()
            .map(|(host, cass_configs)| {
                cass_configs
                    .into_iter()
                    .map(|cass_config| {
                        let cass_config_path_str = cass_config.to_str().unwrap().to_string();
                        let config_file_tuple = if cass_config_path_str.contains("env.sh") {
                            ("cassandra-env", "apache-cassandra/conf/cassandra-env.sh")
                        } else if cass_config_path_str.contains("yaml") {
                            (
                                "cassandra-config-options",
                                "apache-cassandra/conf/cassandra.yaml",
                            )
                        } else {
                            (
                                "jvm11-server-options",
                                "apache-cassandra/conf/jvm11-server.options",
                            )
                        };

                        let task_id = TaskId {
                            cmd: "install".to_string(),
                            task: config_file_tuple.0.to_string(),
                            host: host.clone(),
                        };

                        (
                            task_id.clone(),
                            TaskInstance {
                                task_input: HashMap::from([
                                    (
                                        SOURCE_PATH.to_string(),
                                        TaskArgValue::Str(cass_config_path_str),
                                    ),
                                    (
                                        DEST_PATH.to_string(),
                                        TaskArgValue::Str(config_file_tuple.1.to_string()),
                                    ),
                                ]),
                                task: Box::new(UploadTask::new(config.clone(), task_id)),
                                task_host: TaskHost::Remote {
                                    user: conn_user.clone(),
                                    port: ssh_port as usize,
                                    hosts: host.clone(),
                                },
                            },
                        )
                    })
                    .collect::<IndexMap<TaskId, TaskInstance>>()
            })
            .into_iter()
            .flatten()
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }
}
