use crate::cli::task::task_base::{TaskId, TaskInstance};
use crate::cli::task::upload::upload_task_builder::{
    build_task_instance, get_source_host, UploadTaskBuilder,
};
use crate::config::config_base::{DeployConfig, UploadFile};
use crate::config::DeploymentPackage;
use indexmap::IndexMap;
use itertools::Itertools;

pub struct TxConfUpload;

impl UploadTaskBuilder for TxConfUpload {
    fn build(&self, config: &DeployConfig) -> IndexMap<TaskId, TaskInstance> {
        if config.deployment.tx_service.is_none() {
            return IndexMap::new();
        }

        let all_conf_path = config
            .gen_all_monograph_configs()
            .expect("Failed generate my_HOST.cnf")
            .iter()
            .map(|path_buf| path_buf.to_str().unwrap().to_string())
            .collect_vec();
        let remote_dest = config.deployment.tx_srv_home();
        let upload_cnf_files = config
            .get_host_list(DeploymentPackage::MonographTx)
            .iter()
            .map(|host| {
                let path = all_conf_path
                    .iter()
                    .find_or_first(|path| path.contains(host.as_str()))
                    .unwrap();
                UploadFile {
                    source: path.to_string(),
                    dest: remote_dest.clone(),
                    extension: "cnf".to_string(),
                    host: host.to_string(),
                    copy_dir: false,
                }
            })
            .collect_vec();

        let source_host = get_source_host(None);
        upload_cnf_files
            .iter()
            .map(|upload_file| {
                let host = upload_file.host.clone();
                build_task_instance(
                    source_host.clone(),
                    upload_file.clone(),
                    config,
                    "config-update",
                    format!("upload-cnf-{host}").as_str(),
                )
            })
            .collect::<IndexMap<TaskId, TaskInstance>>()
    }
}
