use crate::cli::download_dir;
use crate::cli::ssh::SSHCommandOption::CollectOutput;
use crate::cli::ssh::SSHSession;
use crate::cli::task::task_base::{
    CmdErr, ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId, TaskInstance,
};
use crate::config::config_base::{
    DeploymentConfig, CASSANDRA_COLLECTOR_AGENT_FILE_KEY, CASSANDRA_FILE_KEY, GRAFANA_FILE_KEY,
    MONOGRAPH_FILE_KEY, MYSQL_EXPORTER_FILE_KEY, NODE_EXPORTER_FILE_KEY, PROMETHEUS_FILE_KEY,
};
use crate::config::{DeploymentService, StorageProvider};
use crate::monitor_component_config_dir;
use crate::task_return_value;
use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

pub(crate) const SOURCE_PATH: &str = "source_file";
pub(crate) const DEST_PATH: &str = "dest_file";
pub(crate) const COPY_DIR: &str = "copy_dir";
pub(crate) const DB_CONFIG_UPLOAD_TASK: &str = "db_config_upload";
pub(crate) const INSTALL_MONOGRAPH_UPLOAD_TASK: &str = "install_monograph_script_upload";
pub(crate) const MONOGRAPH_CONFIG_UPLOAD_TASK: &str = "monograph_config_upload";
pub(crate) const MONOGRAPH_INSTALL_CONFIG_UPLOAD_TASK: &str = "monograph_install_db_conf_upload";

#[derive(Debug, Clone)]
pub struct UploadTask {
    config: DeploymentConfig,
    task_id: TaskId,
}

macro_rules! simple_upload_task_execution {
    ($task_instance:expr,$install_image_files:expr,$file_key:expr,$task:expr,$config:expr,$remote_host:expr, $task_host:expr) => {
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
    ( $({$execution_vec:expr, $task_name:expr, $config:expr, $task_host:expr, $source_path:expr}),*) => {
        $(
        let (_,_,remote_host) =  $task_host.clone().ssh_conn_tuple();
        let task_id = TaskId {
           cmd: "deploy".to_string(),
           task: $task_name,
           host: remote_host,
        };
        $execution_vec.insert(
           task_id.clone(),
           TaskInstance {
               task_input: HashMap::from([(SOURCE_PATH.to_string(),TaskArgValue::Str($source_path))]),
               task: Box::new(UploadTask::new(
                   $config.clone(),task_id)
               ),
               task_host: $task_host.clone(),
           }
        );
        )*
    };
}

macro_rules! monitor_config_upload_task_execution {
    ($config:expr,$component:ident, $monitor_component_host:expr,
     $monitor:expr,$dest_host:expr, $gen_conf_func:expr,$task_name:expr,
     $task_instance_map:expr) => {
        let config_path = $gen_conf_func($dest_host).unwrap();
        if !config_path.is_empty() {
            // let remote_config_path = monitor_remote_config_path!(remote_install_dir, component_name);
            let monitor_host = $monitor_component_host;//&$monitor.$component.host;
            let upload_task_id = TaskId {
                cmd: "deploy".to_string(),
                task: $task_name,
                host: monitor_host.clone(),
            };
            let connection = &$config.connection;
            let remote_host = TaskHost::Remote {
                user: connection.username.to_string(),
                port: connection.ssh_port() as usize,
                hosts: monitor_host.to_string(),
            };
            let component_name = stringify!($component).to_string();
            let dest_file_name = monitor_component_config_dir!(component_name);

            $task_instance_map.insert(
                upload_task_id.clone(),
                TaskInstance {
                    task_input: HashMap::from([
                        (SOURCE_PATH.to_string(), TaskArgValue::Str(config_path)),
                        (DEST_PATH.to_string(), TaskArgValue::Str(dest_file_name)),
                    ]),
                    task: Box::new(UploadTask::new($config.clone(), upload_task_id)),
                    task_host: remote_host,
                },
            );
        }
    };
}
impl UploadTask {
    /// Upload the cassandra.yaml and jvm11-server.options or cassandra-env.sh file to the remote host (remote host list from deployment.yaml).
    pub fn build_upload_cass_conf_task(
        config: &DeploymentConfig,
    ) -> anyhow::Result<IndexMap<TaskId, TaskInstance>> {
        let ssh_port = config.connection.ssh_port();
        let conn_user = config.clone().connection.username;
        let deployment = config.deployment.clone();
        let monitor = deployment.monitor;
        let install_dir = config.install_dir();

        let cass_config = config.deployment.storage_service.gen_cassandra_config(
            install_dir,
            deployment.cluster_name,
            monitor,
        )?;
        let upload_cass_config_task = cass_config
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
            .collect::<IndexMap<TaskId, TaskInstance>>();
        Ok(upload_cass_config_task)
    }

    /// Upload the MonographDB data_dir to the remote host.
    pub fn build_upload_data_dir_tasks(
        config: &DeploymentConfig,
        dest_hosts: Vec<TaskHost>,
    ) -> IndexMap<TaskId, TaskInstance> {
        let datafarm = format!("{}/datafarm", config.install_dir());
        dest_hosts
            .iter()
            .map(|dest_host| {
                let (_, _, host) = dest_host.ssh_conn_tuple();
                let task_id = TaskId {
                    cmd: "install".to_string(),
                    task: "upload_datafarm".to_string(),
                    host,
                };
                (
                    task_id.clone(),
                    TaskInstance {
                        task_input: HashMap::from([
                            (SOURCE_PATH.to_string(), TaskArgValue::Str(datafarm.clone())),
                            (COPY_DIR.to_string(), TaskArgValue::Str("-r".to_string())),
                        ]),
                        task: Box::new(UploadTask::new(config.clone(), task_id)),
                        task_host: dest_host.clone(),
                    },
                )
            })
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }

    pub fn build_upload_mysql_exporter_tasks(
        config: &DeploymentConfig,
    ) -> anyhow::Result<IndexMap<TaskId, TaskInstance>> {
        let tasks = if let Some(monitor) = config.deployment.monitor.as_ref() {
            let monograph_hosts = config.get_host_list(DeploymentService::Monograph);
            let mut task_instances = IndexMap::new();
            let mysql_port = config.deployment.port.mysql_port;
            monograph_hosts.iter().enumerate().for_each(|(idx, host)| {
                if idx == 0 {
                    monitor_config_upload_task_execution!(
                        config,
                        mysql_exporter,
                        host.clone(),
                        monitor,
                        Vec::default(),
                        move |_: Vec<String>| -> anyhow::Result<String> {
                            let create_mysql_user_script = monitor.gen_monitor_user_sql_file()?;
                            Ok(create_mysql_user_script.to_str().unwrap().to_string())
                        },
                        "upload_create_monitor_user_task".to_string(),
                        task_instances
                    );
                }
                monitor_config_upload_task_execution!(
                    config,
                    mysql_exporter,
                    host.clone(),
                    monitor,
                    Vec::default(),
                    move |_: Vec<String>| -> anyhow::Result<String> {
                        let mysql_exporter_config =
                            monitor.gen_mysql_exporter_connect_config(host.clone(), mysql_port)?;
                        Ok(mysql_exporter_config.to_str().unwrap().to_string())
                    },
                    "upload_mysql_exporter_config".to_string(),
                    task_instances
                );
            });
            task_instances
        } else {
            IndexMap::new()
        };
        Ok(tasks)
    }

    pub fn build_upload_monitor_config_tasks(
        config: &DeploymentConfig,
    ) -> anyhow::Result<IndexMap<TaskId, TaskInstance>> {
        let task_instances = if let Some(monitor) = config.deployment.monitor.as_ref() {
            let monograph_hosts = config.get_host_list(DeploymentService::Monograph);
            let mut task_instances = IndexMap::new();

            let prometheus_host = monitor.prometheus.host.to_string();
            monitor_config_upload_task_execution!(
                config,
                prometheus,
                prometheus_host.clone(),
                monitor,
                monograph_hosts,
                |dest_host: Vec<String>| -> anyhow::Result<String> {
                    let config_path = monitor.gen_prometheus_config(dest_host)?;
                    Ok(config_path.to_str().unwrap().to_string())
                },
                "upload_prometheus_config".to_string(),
                task_instances
            );

            let cassandra_hosts = config.get_host_list(DeploymentService::Storage);
            monitor_config_upload_task_execution!(
                config,
                prometheus,
                prometheus_host,
                monitor,
                cassandra_hosts,
                |dest_host: Vec<String>| -> anyhow::Result<String> {
                    let mcac_config = monitor.gen_mcac_file_sd_config(dest_host)?;
                    if let Some(config_path) = mcac_config {
                        Ok(config_path.to_str().unwrap().to_string())
                    } else {
                        Ok("".to_string())
                    }
                },
                "upload_mcac_targets_config".to_string(),
                task_instances
            );

            let grafana_host = &monitor.grafana.host;
            monitor_config_upload_task_execution!(
                config,
                grafana,
                grafana_host.to_string(),
                monitor,
                Vec::default(),
                |_: Vec<String>| -> anyhow::Result<String> {
                    let path = monitor.gen_grafana_datasource_config()?;
                    Ok(path.to_str().unwrap().to_string())
                },
                "upload_prometheus_datasource_config".to_string(),
                task_instances
            );
            task_instances
        } else {
            IndexMap::default()
        };
        Ok(task_instances)
    }

    fn task_instance_from_hosts(
        service: DeploymentService,
        host_vec: Vec<String>,
        config: &DeploymentConfig,
        install_image_files: &HashMap<String, String>,
    ) -> anyhow::Result<IndexMap<TaskId, TaskInstance>> {
        let install_db_script_opt = if service == DeploymentService::Monograph {
            let db_config_pair = config.gen_install_db_script()?;
            Some(db_config_pair)
        } else {
            None
        };

        let install_dir = config.install_dir();
        let install_db_config_path = config
            .deployment
            .gen_monograph_config(None, install_dir.clone())?;

        let install_db_config = install_db_config_path.to_str().unwrap().to_string();
        let conn_user = config.connection.clone().username;
        let ssh_port = config.connection.ssh_port();
        let storage_provider = config.get_monograph_storage()?;
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
                        let mut task_instance = IndexMap::new();
                        if storage_provider == StorageProvider::Cassandra {
                            //$task_instance:expr,$install_image_files:expr,$file_key:expr,$task:expr,$config:expr,$task_host:expr
                            simple_upload_task_execution!(
                                task_instance,
                                install_image_files,
                                CASSANDRA_FILE_KEY,
                                "cassandra_upload".to_string(),
                                config,
                                remote_host.clone(),
                                task_host
                            );
                            simple_upload_task_execution!(
                                task_instance,
                                install_image_files,
                                CASSANDRA_COLLECTOR_AGENT_FILE_KEY,
                                "cassandra_collector_agent_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                            );
                        }
                        task_instance
                    }
                    DeploymentService::Monograph => {
                        info!("UploadTask upload file to remote={:?}", task_host);
                        let mut task_execution_vec:IndexMap<TaskId, TaskInstance> = IndexMap::new();
                        let db_config_path =
                            config.clone().deployment.gen_monograph_config(Some(remote_host.clone()), install_dir.clone()).unwrap();

                        let install_db_script = install_db_script_opt.as_ref().unwrap().clone();
                        let db_config_path_str = db_config_path.to_str().unwrap().to_string();
                        // $execution_vec:expr, $task_name:expr, $config:expr, $task_host:expr, $source_path:expr
                        let install_db_path_string = install_db_script.to_str().unwrap().to_string();

                        let monograph_download_file =
                            install_image_files.get(MONOGRAPH_FILE_KEY).unwrap();

                        let monograph_download_location = download_dir().
                            join(monograph_download_file).to_str().
                            unwrap().to_string();
                        monograph_config_task_execution! {
                            {task_execution_vec, DB_CONFIG_UPLOAD_TASK.to_string(), config, task_host, db_config_path_str},
                            {task_execution_vec, INSTALL_MONOGRAPH_UPLOAD_TASK.to_string(), config, task_host, install_db_path_string},
                            {task_execution_vec, MONOGRAPH_CONFIG_UPLOAD_TASK.to_string(), config, task_host, monograph_download_location},
                            {task_execution_vec, MONOGRAPH_INSTALL_CONFIG_UPLOAD_TASK.to_string(), config, task_host, install_db_config.clone()}
                        }

                        simple_upload_task_execution!(
                                task_execution_vec,
                                install_image_files,
                                NODE_EXPORTER_FILE_KEY,
                                "node_exporter_upload".to_string(),
                                config,
                                remote_host.clone(),
                                task_host
                        );

                        simple_upload_task_execution!(
                                task_execution_vec,
                                install_image_files,
                                MYSQL_EXPORTER_FILE_KEY,
                                "mysql_exporter_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                        );
                        task_execution_vec
                    }
                    DeploymentService::Prometheus => {
                        let mut task_instance = IndexMap::new();
                        simple_upload_task_execution!(
                                task_instance,
                                install_image_files,
                                PROMETHEUS_FILE_KEY,
                                "prometheus_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                        );
                        task_instance
                    }
                    DeploymentService::Grafana => {
                        let mut task_instance = IndexMap::new();
                        simple_upload_task_execution!(
                                task_instance,
                                install_image_files,
                                GRAFANA_FILE_KEY,
                                "grafana_upload".to_string(),
                                config,
                                remote_host,
                                task_host
                       );
                       task_instance
                    }
                }
            })
            .into_iter()
            .flatten()
            .collect::<IndexMap<TaskId, TaskInstance>>();
        Ok(execution_context_vec)
    }

    /// Upload installation package, MonographDB configuration file (my.cnf),
    /// MonographDB install script, install config to remote host.
    pub fn from_config(
        config: &DeploymentConfig,
    ) -> anyhow::Result<IndexMap<TaskId, TaskInstance>> {
        let all_hosts = config.get_host_as_map();
        let install_image_files_map = config.install_image_files()?;
        let upload_task_instance = all_hosts
            .into_iter()
            .map(|entry| {
                let service = entry.0;
                let hosts = entry.1;
                UploadTask::task_instance_from_hosts(
                    service,
                    hosts,
                    config,
                    &install_image_files_map,
                )
                .unwrap()
            })
            .into_iter()
            .flatten()
            .collect::<IndexMap<TaskId, TaskInstance>>();

        Ok(upload_task_instance)
    }

    pub fn new(config: DeploymentConfig, task_id: TaskId) -> Self {
        Self { config, task_id }
    }

    pub async fn create_remote_directory(&self, remote_task_host: TaskHost) -> anyhow::Result<()> {
        let ssh_session = SSHSession::from_task_host(
            remote_task_host,
            self.config.connection.ssh_auth_key().unwrap(),
        )
        .await?;
        let mkdir = format!("mkdir -p {}", self.config.install_dir());
        let mkdir_output = ssh_session.command(mkdir.as_str(), CollectOutput).await?;
        info!("UploadTask create remote dir complete={:?}", mkdir_output);
        Ok(())
    }
}

#[async_trait]
impl TaskExecutor for UploadTask {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        remote_task_host: TaskHost,
        task_input: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        println!("{} execute.\n", self.task_id.pretty_string());
        self.create_remote_directory(remote_task_host.clone())
            .await?;

        let source_ip_rs = local_ip_address::local_ip()?;
        let local_ip_addr = source_ip_rs.to_string();
        let ssh_port = self.config.connection.ssh_port();
        let ssh_user = self.config.connection.clone().username;
        let source_task_host = TaskHost::Remote {
            user: ssh_user,
            port: ssh_port as usize,
            hosts: local_ip_addr,
        };

        let ssh_session = SSHSession::from_task_host(
            source_task_host,
            self.config.connection.ssh_auth_key().unwrap(),
        )
        .await?;

        let remote_install_dir = self.config.install_dir();

        let (remote_user, port, remote_host) = remote_task_host.ssh_conn_tuple();
        let source_path_str =
            TaskArgValue::into_inner_value::<String>(task_input.get(SOURCE_PATH).unwrap().clone());

        let source_path_buf = PathBuf::from(source_path_str.as_str());

        let dest_file_name = if let Some(dest_file_str) = task_input.get(DEST_PATH) {
            TaskArgValue::into_inner_value::<String>(dest_file_str.clone())
        } else {
            source_path_buf
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        };

        let copy_dir = if let Some(scp_dir) = task_input.get(COPY_DIR) {
            TaskArgValue::into_inner_value::<String>(scp_dir.clone())
        } else {
            "".to_string()
        };

        let scp_auth_key = format!("-i {}", self.config.connection.ssh_auth_key().unwrap());
        // scp /xxx/local_file user@remote_host:remote_dir/file
        let scp_cmd = format!(
            // dir port, usr host remote_dir file_name
            r#"scp -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no {copy_dir} {scp_auth_key} -P {port} {source_path_str} {remote_user}@{remote_host}:{remote_install_dir}/{dest_file_name}"#,
        );
        info!("UploadTask cmd={}", scp_cmd);
        let err_msg = format!("cmd={scp_cmd},source_path={source_path_str}");
        let task_rs = ssh_session.command(scp_cmd.as_str(), CollectOutput).await?;
        ssh_session.close().await?;
        task_return_value!(
            task_rs,
            |status_code: usize| -> CmdErr { CmdErr::UploadErr(err_msg, status_code.to_string()) },
            "UploadTask"
        );
    }
}
