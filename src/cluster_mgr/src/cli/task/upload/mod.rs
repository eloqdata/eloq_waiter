mod data_dir_upload_builder;
mod monitor_upload_builder;
mod mono_service_upload_builder;
mod storage_upload_builder;
pub mod upload_task;
pub mod upload_task_builder;

pub(crate) const SOURCE_PATH: &str = "source_file";
pub(crate) const DEST_PATH: &str = "dest_file";
pub(crate) const SOURCE_HOST: &str = "source_host";
pub(crate) const COPY_DIR: &str = "copy_dir";
pub(crate) const DB_CONFIG_UPLOAD_TASK: &str = "db_config_upload";
pub(crate) const INSTALL_MONOGRAPH_UPLOAD_TASK: &str = "install_monograph_script_upload";
pub(crate) const MONOGRAPH_CONFIG_UPLOAD_TASK: &str = "monograph_config_upload";
pub(crate) const MONOGRAPH_INSTALL_CONFIG_UPLOAD_TASK: &str = "monograph_install_db_conf_upload";

pub(crate) const LOG_INSTALL_UPLOAD_TASK: &str = "monograph_log_upload";
pub(crate) const LOG_START_CMD_UPLOAD_TASK: &str = "monograph_start_cmd_upload";
