use crate::cli::task::task_base::{TaskArgValue, TaskHost, TaskId, TaskInstance};
use crate::cli::task::upload::upload_task::UploadTask;
use crate::cli::task::upload::upload_task_builder::UploadTaskBuilder;
use crate::cli::task::upload::*;
use crate::config::config_base::DeploymentConfig;
use crate::config::DeploymentPackage;
use crate::monitor_component_config_dir;
use indexmap::IndexMap;
use std::collections::HashMap;

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

pub struct MySQLExporterUploadBuilder;
pub struct MonitorInfraConfUploadBuilder;

impl UploadTaskBuilder for MySQLExporterUploadBuilder {
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance> {
        let tasks = if let Some(monitor) = config.deployment.monitor.as_ref() {
            let monograph_hosts = config.get_host_list(DeploymentPackage::MonographTx);
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
            IndexMap::default()
        };
        tasks
    }
}

impl UploadTaskBuilder for MonitorInfraConfUploadBuilder {
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance> {
        let task_instances = if let Some(monitor) = config.deployment.monitor.as_ref() {
            let monograph_hosts = config.get_host_list(DeploymentPackage::MonographTx);
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

            let cassandra_hosts = config.get_host_list(DeploymentPackage::Storage);
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

            let grafana = &monitor.grafana;
            let grafana_host = &grafana.host;
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
        task_instances
    }
}
