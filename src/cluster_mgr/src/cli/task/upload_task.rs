use crate::cli::config::{DeploymentConfig, DeploymentService};
use crate::cli::download_dir;
use crate::cli::task::task_base::{
    ExecutionResult, TaskExecutionContext, TaskExecutor, TaskHost, TaskId, TaskValue,
};
use crate::ssh_conn_info;
use async_trait::async_trait;
use itertools::Itertools;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

pub(crate) const SOURCE_PATH: &str = "source_file";

#[derive(Clone)]
pub struct UploadTask {
    // source_path: PathBuf,
    config: DeploymentConfig,
    task_host: TaskHost,
    task_id: TaskId,
}
#[macro_export]
macro_rules! monograph_config_task_execution {
    ( $({$execution_vec:expr, $task_name:expr, $config:expr, $task_host:expr, $source_path:expr}),*) => {
        $(
        $execution_vec.push(
           TaskExecutionContext {
           task_input: HashMap::from([(SOURCE_PATH.to_string(),TaskValue::Str($source_path))]),
           task: Box::new(UploadTask::new(
               $config.clone(),
               $task_host.clone(),
               TaskId {
                   cmd: "deploy".to_string(),
                   task: format!("{}_upload", $task_name),
               },)
           ),
           task_host: $task_host.clone(),
        }
        );
        )*
    };
}

impl UploadTask {
    fn tasks_from_host_list(
        service: DeploymentService,
        host_vec: Vec<String>,
        config: &DeploymentConfig,
    ) -> anyhow::Result<Vec<TaskExecutionContext>> {
        let download_files = config.download_file_as_map()?;
        let db_start_script_path = if service == DeploymentService::Monograph {
            Some(config.gen_db_start_script()?)
        } else {
            None
        };
        let conn_user = config.connection.clone().username;
        let ssh_port = config.connection.ssh_port();
        let execution_context_vec = host_vec
            .into_iter()
            .map(|remote_host| {
                let task_host = TaskHost::Remote {
                    user: conn_user.clone(),
                    hosts: remote_host.clone(),
                    port: ssh_port as usize,
                };
                match service {
                    DeploymentService::Storage => {
                        let cassandra_download_file =
                            download_files.get(&DeploymentService::Storage).unwrap();

                        let cassandra_download_path = download_dir()
                            .join(cassandra_download_file)
                            .to_str()
                            .unwrap()
                            .to_string();

                        let upload_cassandra = UploadTask::new(
                            config.clone(),
                            task_host.clone(),
                            TaskId {
                                cmd: "deploy".to_string(),
                                task: format!("{}_upload", "cassandra"),
                            },
                        );
                        vec![TaskExecutionContext {
                            task_input: HashMap::from([(
                                SOURCE_PATH.to_string(),
                                TaskValue::Str(cassandra_download_path),
                            )]),
                            task: Box::new(upload_cassandra),
                            task_host,
                        }]
                    }
                    DeploymentService::Monograph => {
                        let mut task_execution_vec = vec![];
                        let db_config_path =
                            config.clone().gen_monograph_config(remote_host).unwrap();

                        let db_config_path_str = db_config_path.to_str().unwrap().to_string();
                        // $execution_vec:expr, $task_name:expr, $config:expr, $task_host:expr, $source_path:expr
                        let start_db_script = db_start_script_path.clone().unwrap();
                        let start_db_script_path_string = start_db_script.to_str().unwrap().to_string();

                        let monograph_download_file =
                             download_files.get(&DeploymentService::Monograph).unwrap();

                        let monograph_download_location = download_dir().
                            join(monograph_download_file).to_str().
                            unwrap().to_string();
                        monograph_config_task_execution! {
                            {task_execution_vec, "db_config", config, task_host, db_config_path_str},
                            {task_execution_vec, "db_script", config, task_host, start_db_script_path_string},
                            {task_execution_vec, "monograph", config, task_host, monograph_download_location}
                        }
                        task_execution_vec
                    }
                }
            })
            .into_iter()
            .flatten()
            .collect_vec();
        Ok(execution_context_vec)
    }

    pub fn from_config(config: &DeploymentConfig) -> anyhow::Result<Vec<TaskExecutionContext>> {
        let all_hosts = config.get_host_as_map();
        let execution_context_vec = all_hosts
            .into_iter()
            .map(|entry| {
                let service = entry.0;
                let hosts = entry.1;
                UploadTask::tasks_from_host_list(service, hosts, config).unwrap()
            })
            .into_iter()
            .flatten()
            .collect_vec();

        Ok(execution_context_vec)
    }

    pub fn new(
        // source_path: PathBuf,
        config: DeploymentConfig,
        task_host: TaskHost,
        task_id: TaskId,
    ) -> Self {
        Self {
            config,
            task_host,
            task_id,
        }
    }
}

#[async_trait]
impl TaskExecutor for UploadTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        task_input: HashMap<String, TaskValue>,
    ) -> anyhow::Result<Option<ExecutionResult>> {
        ssh_conn_info! {
            self.config.connection.clone(),
            task_host,
            ssh_conn,
            conn_user,
            conn_host
        }
        let source_path_str =
            TaskValue::into_inner_value::<String>(task_input.get(SOURCE_PATH).unwrap().clone());

        let source_path_buf = PathBuf::from(source_path_str.as_str());
        // scp /xxx/local_file user@remote_host:remote_dir/file
        let remote_install_dir = self.config.install_dir();
        let ssh_port = self.config.connection.ssh_port();
        //let local_file = self.source_path.to_str().unwrap();
        info!(
            "UploadFileTask will be start local_file={}",
            source_path_str
        );
        let source_file_name = source_path_buf.file_name().unwrap().to_str().unwrap();
        let scp_cmd = format!(
            // dir port, usr host remote_dir file_name
            r#"mkdir -p {} && scp -P {} {} {}@{}:{}/{}"#,
            remote_install_dir,
            ssh_port,
            source_path_str,
            conn_user,
            conn_host,
            remote_install_dir,
            source_file_name
        );
        let task_rs = ssh_conn?.run_cmd(scp_cmd.clone(), false);
        info!(
            "UploadFileTask complete cmd={}, result={:?}",
            scp_cmd, task_rs
        );
        Ok(None)
    }

    // async fn post_execute(
    //     &self,
    //     execution_rs: anyhow::Result<Option<ExecutionResult>>,
    // ) -> anyhow::Result<Option<ExecutionResult>> {
    //     let conn_tuple = self.task_host.ssh_conn_tuple();
    //     let task_id_string = self.task_id.as_string();
    //     task_execute_post!(
    //         execution_rs,
    //         self.config.deployment.clone().cluster_name,
    //         task_id_string,
    //         "deploy",
    //         conn_tuple.2
    //     )
    // }
}
