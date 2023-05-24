use crate::cli::task::task_base::{TaskId, TaskInstance};
use crate::cli::task::upload::upload_task_builder::{
    build_task_instance, get_source_host, UploadTaskBuilder,
};
use crate::config::config_base::{DeploymentConfig, UploadFile};
use crate::config::monitor::{
    Monitor, GRAFANA_CONFIG_DIR, GRAFANA_DASHBOARD_CONFIG_DIR, GRAFANA_DATASOURCE_CONFIG_DIR,
    PROMETHEUS_CONFIG_DIR,
};
use crate::config::DeploymentPackage;
use indexmap::IndexMap;
use itertools::Itertools;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone)]
struct ConfigAndHostPair {
    path: Vec<PathBuf>,
    hosts: Vec<String>,
}

pub struct MonitorInfraConfUploadBuilder;

impl MonitorInfraConfUploadBuilder {
    fn monitor_upload_files(
        &self,
        all_monitor_config: HashMap<String, ConfigAndHostPair>,
    ) -> Vec<UploadFile> {
        all_monitor_config
            .iter()
            .flat_map(|(dest_dir, path_and_hosts)| {
                //let monitor_conf_tmp_dir = create_temp_dir(tmp_prefix, "/tmp").unwrap();
                let path_vec = &path_and_hosts.path;
                let path_files = path_vec
                    .iter()
                    .map(|path| path.to_str().unwrap())
                    .map(|path_str| path_str.to_string())
                    .join(" ");

                let hosts = &path_and_hosts.hosts;
                hosts
                    .iter()
                    .map(|host| UploadFile {
                        source: path_files.clone(),
                        dest: dest_dir.clone(),
                        extension: "".to_string(),
                        host: host.to_string(),
                        copy_dir: false,
                    })
                    .collect_vec()
            })
            .collect_vec()
    }

    fn dashboard_upload_files(&self, config: &DeploymentConfig) -> Option<UploadFile> {
        let files = config.load_monitor_dashboard(None);
        if files.is_empty() {
            None
        } else {
            let host = config.get_host_list(DeploymentPackage::Grafana);
            assert_eq!(1, host.len());
            let dest_host = host.first().unwrap();
            let dashboard_files = files.iter().join(" ");
            let install_dir = config.install_dir();
            Some(UploadFile {
                source: dashboard_files,
                dest: format!("{install_dir}/{GRAFANA_DASHBOARD_CONFIG_DIR}"),
                extension: "json".to_string(),
                host: dest_host.to_string(),
                copy_dir: false,
            })
        }
    }

    fn gen_monitor_config(
        &self,
        monitor: &Monitor,
        config: &DeploymentConfig,
    ) -> HashMap<String, ConfigAndHostPair> {
        let all_host = config.get_host_as_map();
        let install_dir = config.install_dir();
        let monograph_tx_hosts_ref = all_host.get(&DeploymentPackage::MonographTx).unwrap();
        let cass_config_host_ref = all_host.get(&DeploymentPackage::Storage).unwrap();
        let monograph_tx_hosts = monograph_tx_hosts_ref.clone();

        let create_user_script = monitor.gen_monitor_user_sql_file().unwrap(); //install_dir
        let grafana_ds_conf_path = monitor.gen_grafana_datasource_config().unwrap(); //grafana datasource
        let grafana_conf_path = monitor.gen_grafana_config().unwrap(); //grafana
        let mcac_config = monitor
            .gen_mcac_file_sd_config(cass_config_host_ref.clone())
            .unwrap(); // prometheus
        let prometheus_conf = monitor.gen_prometheus_config(monograph_tx_hosts).unwrap(); //prometheus config
        let prometheus_files = if let Some(mcac) = mcac_config {
            vec![prometheus_conf, mcac]
        } else {
            vec![prometheus_conf]
        };
        // key is dest dir, value is source path
        HashMap::from([
            (
                format!("{install_dir}/{PROMETHEUS_CONFIG_DIR}"),
                ConfigAndHostPair {
                    path: prometheus_files,
                    hosts: all_host
                        .get(&DeploymentPackage::Prometheus)
                        .unwrap()
                        .clone(),
                },
            ),
            (
                format!("{install_dir}/{GRAFANA_CONFIG_DIR}"),
                ConfigAndHostPair {
                    path: vec![grafana_conf_path],
                    hosts: all_host.get(&DeploymentPackage::Grafana).unwrap().clone(),
                },
            ),
            (
                format!("{install_dir}/{GRAFANA_DATASOURCE_CONFIG_DIR}"),
                ConfigAndHostPair {
                    path: vec![grafana_ds_conf_path],
                    hosts: all_host.get(&DeploymentPackage::Grafana).unwrap().clone(),
                },
            ),
            (
                install_dir,
                ConfigAndHostPair {
                    path: vec![create_user_script],
                    hosts: all_host
                        .get(&DeploymentPackage::MonographTx)
                        .unwrap()
                        .clone(),
                },
            ),
        ])
    }
}

impl UploadTaskBuilder for MonitorInfraConfUploadBuilder {
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance> {
        let monitor_opt = config.deployment.monitor.as_ref();
        let source_host = get_source_host(None);
        if let Some(monitor) = monitor_opt {
            let all_monitor_config = self.gen_monitor_config(monitor, config);
            let mut all_upload_files = self.monitor_upload_files(all_monitor_config);
            if let Some(upload_dashboard_file) = self.dashboard_upload_files(config) {
                all_upload_files.push(upload_dashboard_file);
            }
            all_upload_files
                .iter()
                .map(|upload_file| {
                    build_task_instance(
                        source_host.clone(),
                        upload_file.clone(),
                        config,
                        "deploy",
                        "upload_monitor_cnf",
                    )
                })
                .collect::<IndexMap<TaskId, TaskInstance>>()
        } else {
            IndexMap::new()
        }
    }
}
