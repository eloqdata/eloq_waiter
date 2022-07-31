use crate::config::common::Common;
use ini::{Ini, Properties};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::path::Path;

pub mod common;

pub enum ConfigObject {
    Common(Common),
    MySQL(Properties),
}

pub fn load_config(base_config: &str) -> &'static HashMap<String, ConfigObject> {
    static INSTANCE: OnceCell<HashMap<String, ConfigObject>> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        let mut config_mapping = HashMap::new();
        let common_config_str = format!("{}/{}", base_config, "/common.toml");
        let common_object = load_common_config(common_config_str.as_str());
        config_mapping.insert("common".to_string(), ConfigObject::Common(common_object));

        let mysql_config_str = format!("{}/{}", base_config, "/mysql/mysql_template.cnf");
        let properties = load_mysql_config(mysql_config_str.as_str());
        config_mapping.insert("mysql".to_string(), ConfigObject::MySQL(properties));
        config_mapping
    })
}

fn load_common_config(common_config_path: &str) -> Common {
    let common_binary = std::fs::read(Path::new(common_config_path))
        .unwrap_or_else(|_| panic!("can't read common.toml from path = {}", common_config_path));
    let common: Common = toml::from_slice(&common_binary).unwrap();
    common
}

fn load_mysql_config(mysql_config_path: &str) -> Properties {
    let my_cnf = Ini::load_from_file(mysql_config_path).unwrap();
    my_cnf.section(Some("mariadb")).unwrap().clone()
}

#[cfg(test)]
mod tests {
    use crate::config::{load_common_config, load_mysql_config};

    pub fn config_file(file: &str) -> String {
        let mut base_path = env!("CARGO_MANIFEST_DIR").to_owned();
        base_path.push_str(file);
        base_path
    }

    #[test]
    pub fn test_load_common_config() {
        let common_path = config_file("/config/common.toml");
        let common_rs = load_common_config(common_path.as_str());
        println!("{:?}", common_rs);
    }

    #[test]
    pub fn test_load_mysql_config() {
        let mysql_config_path = config_file("/config/mysql/mysql_template.cnf");
        load_mysql_config(mysql_config_path.as_str());
    }
}
