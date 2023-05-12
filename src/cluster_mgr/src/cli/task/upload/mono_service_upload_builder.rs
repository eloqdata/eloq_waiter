use crate::cli::download_dir;
use crate::cli::task::task_base::{TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::task::upload::upload_task::UploadTask;
use crate::cli::task::upload::upload_task_builder::UploadTaskBuilder;
use crate::cli::task::upload::*;
use crate::config::config_base::{
    DeploymentConfig, CASSANDRA_COLLECTOR_AGENT_FILE_KEY, CASSANDRA_FILE_KEY, GRAFANA_FILE_KEY,
    MONOGRAPH_FILE_KEY, MONOGRAPH_LOG_FILE_KEY, MYSQL_EXPORTER_FILE_KEY, NODE_EXPORTER_FILE_KEY,
    PROMETHEUS_FILE_KEY,
};

use crate::config::{DeploymentPackage, StorageProvider};
use indexmap::IndexMap;
use itertools::Itertools;
use std::collections::HashMap;
use tracing::info;

macro_rules! simple_upload_task_execution {
    ($task_instance:expr,$install_image_files:expr,
     $file_key:expr,$task:expr,$config:expr,
     $remote_host:expr,$task_host:expr) => {
        if let Some(download_file) = $install_image_files.get($file_key) {
            let download_path = download_dir()
                .join(download_file.to_string())
                .to_str()
                .unwrap()
                .to_string();

            let task_id = TaskId {
                cmd: "deploy".to_string(),
                task: $task,
                host: $remote_host,
            };

            let upload_task = UploadTask::new($config.clone(), task_id.clone());

            $task_instance.insert(
                task_id,
                TaskInstance {
                    task_input: HashMap::from([(
                        SOURCE_PATH.to_string(),
                        TaskArgValue::Str(download_path),
                    )]),
                    task: Box::new(upload_task),
                    task_host: $task_host.clone(),
                },
            );
        }
    };
}


#[macro_export]
macro_rules! monograph_config_task_execution {
    ( $({$execution_vec:expr, $task_name:expr, $config:expr, $task_host:expr, $source_path:expr $(,$dest_path:expr)?}),*) => {
        $(
        let (_,_,remote_host) =  $task_host.clone().ssh_conn_tuple();
        let task_id = TaskId {
           cmd: "deploy".to_string(),
           task: $task_name,
           host: remote_host,
        };
        #[allow(unused_mut)]
        let mut task_input = HashMap::from([(SOURCE_PATH.to_string(),TaskArgValue::Str($source_path))]);
        $(
          task_input.insert(DEST_PATH.to_string(), TaskArgValue::Str($dest_path));
        )?
        $execution_vec.insert(
           task_id.clone(),
           TaskInstance {
               task_input,
               task: Box::new(UploadTask::new(
                   $config.clone(),task_id)
               ),
               task_host: $task_host.clone(),
           }
        );
        )*
    };
}

pub struct MonographUploadBuilder;

impl MonographUploadBuilder {
    fn build_task_from_args(
        &self,
        package: DeploymentPackage,
        service_hosts: Vec<String>,
        config: &DeploymentConfig,
        install_image_files: &HashMap<String, String>,
    ) -> IndexMap<TaskId, TaskInstance> {
        let monograph_script_opt = if package == DeploymentPackage::MonographTx {
            let db_config_pair = config.gen_bootstrap_db_script().unwrap();
            Some(vec![db_config_pair])
        } else if package == DeploymentPackage::MonographLog {
            config.gen_log_start_script().unwrap()
        } else {
            None
        };
        let install_dir = config.install_dir();
        let conn_user = config.connection.clone().username;
        let ssh_port = config.connection.ssh_port();
        let storage_provider = config.get_monograph_storage().unwrap();
        service_hosts
            .into_iter()
            .map(|remote_host| {
                let task_host = TaskHost::Remote {
                    user: conn_user.clone(),
                    hosts: remote_host.clone(),
                    port: ssh_port as usize,
                };
                match package {
                    DeploymentPackage::Storage => {
                        let mut upload_cass_tasks = IndexMap::new();
                        if storage_provider == StorageProvider::Cassandra {
                            //$task_instance:expr,$install_image_files:expr,$file_key:expr,$task:expr,$config:expr,$task_host:expr
                            simple_upload_task_execution!(
                                upload_cass_tasks,
                                install_image_files,
                                CASSANDRA_FILE_KEY,
                                "cassandra_upload".to_string(),
                                config,
                                remote_host.clone(),
                                task_host
                            );
                            simple_upload_task_execution!(
                                upload_cass_tasks,
                                install_image_files,
                                CASSANDRA_COLLECTOR_AGENT_FILE_KEY,
                                "cassandra_collector_agent_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                            );
                        }
                        upload_cass_tasks
                    }
                    DeploymentPackage::MonographLog => {
                        let log_install_file = install_image_files.get(MONOGRAPH_LOG_FILE_KEY).unwrap();
                        let log_install_file_path =  download_dir().
                            join(log_install_file).to_str().
                            unwrap().to_string();

                        let all_log_start_cmd = monograph_script_opt.as_ref().unwrap();
                        let log_cmd_path= all_log_start_cmd.iter().filter(|path| {
                            let log_start_script = path.file_name().unwrap().to_str().unwrap();
                            log_start_script.contains(remote_host.as_str())
                        }).map(|path| path.to_str().unwrap().to_string()).collect_vec();

                        let mut upload_log_tasks = IndexMap::new();
                        let log_home_dir = config.log_home_dir();
                        //let log_home_dir  = log_home_dir_binding.as_str();

                        log_cmd_path.iter().for_each(|cmd_path| {
                            monograph_config_task_execution! {
                                {upload_log_tasks, LOG_START_CMD_UPLOAD_TASK.to_string(), config, task_host, cmd_path.to_string(), log_home_dir.clone()}
                            }
                        });
                        monograph_config_task_execution! {
                            {upload_log_tasks, LOG_INSTALL_UPLOAD_TASK.to_string(), config, task_host, log_install_file_path, log_home_dir.clone()}
                        }
                        upload_log_tasks
                    }
                    DeploymentPackage::MonographTx => {
                        info!("UploadTask upload TxService file to remote={:?}", task_host);
                        let mut upload_tx_tasks = IndexMap::new();
                        let db_config_path =
                            config.deployment.gen_monograph_config(Some(remote_host.clone()), install_dir.clone()).unwrap();
                        let db_config_path_string = db_config_path.to_str().unwrap().to_string();

                        let bootstrap_db_script = monograph_script_opt.as_ref().unwrap().first().unwrap();
                        // $execution_vec:expr, $task_name:expr, $config:expr, $task_host:expr, $source_path:expr
                        let bootstrap_db_path_string = bootstrap_db_script.to_str().unwrap().to_string();

                        let tx_install_file =
                            install_image_files.get(MONOGRAPH_FILE_KEY).unwrap();

                        let tx_install_tar_path = download_dir().
                            join(tx_install_file).to_str().
                            unwrap().to_string();
                        monograph_config_task_execution! {
                            {upload_tx_tasks, DB_CONFIG_UPLOAD_TASK.to_string(), config, task_host, db_config_path_string.clone()},
                            {upload_tx_tasks, INSTALL_MONOGRAPH_UPLOAD_TASK.to_string(), config, task_host, bootstrap_db_path_string},
                            {upload_tx_tasks, MONOGRAPH_CONFIG_UPLOAD_TASK.to_string(), config, task_host, tx_install_tar_path},
                            {upload_tx_tasks, MONOGRAPH_INSTALL_CONFIG_UPLOAD_TASK.to_string(), config, task_host, db_config_path_string}
                        }

                        simple_upload_task_execution!(
                                upload_tx_tasks,
                                install_image_files,
                                NODE_EXPORTER_FILE_KEY,
                                "node_exporter_upload".to_string(),
                                config,
                                remote_host.clone(),
                                task_host
                        );

                        simple_upload_task_execution!(
                                upload_tx_tasks,
                                install_image_files,
                                MYSQL_EXPORTER_FILE_KEY,
                                "mysql_exporter_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                        );
                        upload_tx_tasks
                    }
                    DeploymentPackage::Prometheus => {
                        let mut upload_prometheus_tasks = IndexMap::new();
                        simple_upload_task_execution!(
                                upload_prometheus_tasks,
                                install_image_files,
                                PROMETHEUS_FILE_KEY,
                                "prometheus_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                        );
                        upload_prometheus_tasks
                    }
                    DeploymentPackage::Grafana => {
                        let mut upload_grafana_tasks = IndexMap::new();
                        simple_upload_task_execution!(
                                upload_grafana_tasks,
                                install_image_files,
                                GRAFANA_FILE_KEY,
                                "grafana_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                       );
                        upload_grafana_tasks
                    }
                }
            })
            .into_iter()
            .flatten()
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }
}

impl UploadTaskBuilder for MonographUploadBuilder {
    /// Upload installation package, MonographDB configuration file (my.cnf),
    /// MonographDB install script, install config to remote host.
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance> {
        let all_hosts = config.get_host_as_map();
        let install_image_files_map_rs = config.install_image_files();
        assert!(install_image_files_map_rs.is_ok());
        let install_image_files_map = install_image_files_map_rs.unwrap();
        all_hosts
            .into_iter()
            .map(|entry| {
                let service = entry.0;
                let hosts = entry.1;
                self.build_task_from_args(service, hosts, config, &install_image_files_map)
            })
            .into_iter()
            .flatten()
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }
}
