use crate::cli::task::task_base::{TaskId, TaskInstance};
use crate::cli::task::upload::data_dir_upload_builder::DataDirUploadBuilder;
use crate::cli::task::upload::monitor_upload_builder::*;
use crate::cli::task::upload::mono_service_upload_builder::MonographUploadBuilder;
use crate::cli::task::upload::storage_upload_builder::CassConfUploadBuilder;
use crate::config::config_base::DeploymentConfig;
use indexmap::IndexMap;

pub trait UploadTaskBuilder {
    /// During the deployment phase, it is necessary to generate the corresponding upload execution tasks based on the deployment.yaml.
    /// These tasks include but are not limited to:
    /// 1. Uploading MonographDB TxService, including configuration files and bootstrap database commands for each instance.
    /// 2. Uploading MonographDB LogService, including start-up commands for each instance (if configured).
    /// 3. Uploading the Monitor component, including (NodeExporter, MySQLExporter, Prometheus, Grafana, Cassandra Monitor)
    ///    and configuration files for all components.
    /// 4. Modifying and uploading the configuration files for Cassandra config (cassandra.yml, jvm11-server.options).
    fn build(&self, config: &DeploymentConfig) -> IndexMap<TaskId, TaskInstance>;
}

#[derive(Clone, Debug)]
pub enum UploadTaskBuilderType {
    MySQLExporter,
    CassConf,
    DataDir,
    InstallTar,
    MonitorConf,
}

#[macro_export]
macro_rules! build_upload_tasks {
    ($builder_impl:ident, $conf: expr) => {{
        $builder_impl {}.build($conf)
    }};
}

pub fn upload_tasks(
    builder_type: UploadTaskBuilderType,
    conf: &DeploymentConfig,
) -> IndexMap<TaskId, TaskInstance> {
    match builder_type {
        UploadTaskBuilderType::MySQLExporter => MySQLExporterUploadBuilder {}.build(conf),
        UploadTaskBuilderType::CassConf => CassConfUploadBuilder {}.build(conf),
        UploadTaskBuilderType::DataDir => DataDirUploadBuilder {}.build(conf),
        UploadTaskBuilderType::InstallTar => MonographUploadBuilder {}.build(conf),
        UploadTaskBuilderType::MonitorConf => MonitorInfraConfUploadBuilder {}.build(conf),
    }
}
