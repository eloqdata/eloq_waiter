use once_cell::sync::Lazy;
use std::collections::HashMap;
use sysinfo::SystemExt;

pub static SYSTEM_DEPS: &[&str; 2] = &["darwin", "ubuntu"];
pub static MONO_WATER_CONF: &str = "MONO_WATER_CONF_DIR";
pub static PROTOBUF_TAR_FILE_NAME: &str = "protobuf-bin.tar.gz";
pub static CASSANDRA_TAR_FILE_NAME: &str = "cassandra-bin.tar.gz";

pub static SUPPORT_CMD_LIST: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "check_deps",
        "install_deps",
        "setup_workspace",
        "ln_source",
        "gen_mysql_cnf",
        "build",
        "playground",
        "stop_all",
        "start_all",
    ]
});

pub static SYSTEM_INFO: Lazy<HashMap<&'static str, String>> = Lazy::new(|| {
    let mut sys_info_map = HashMap::new();
    let system = sysinfo::System::new_all();
    sys_info_map.insert("os_type", system.name().unwrap().to_lowercase());
    sys_info_map.insert("os_version", system.os_version().unwrap());
    sys_info_map
});
