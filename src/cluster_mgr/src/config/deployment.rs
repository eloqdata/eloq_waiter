use crate::cli::download_dir;

use crate::config::monitor::Monitor;
use crate::config::storage_service_config::StorageService;
use crate::config::{
    config_template, CONFIG_MARIADB_SECTION, MONOGRAPH_CONF_DYNAMO_TEMPLATE,
    MONOGRAPH_CONF_TEMPLATE,
};
use anyhow::anyhow;
use configparser::ini::Ini;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Port {
    pub mysql_port: u16,
    pub monograph_port: MonographPort,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MonographPort {
    pub start: u16,
    pub end: u16,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MonographService {
    pub host: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Deployment {
    pub install_image: String,
    pub cluster_name: String,
    pub install_dir: String,
    pub port: Port,
    pub mono_service: MonographService,
    pub storage_service: StorageService,
    pub monitor: Option<Monitor>,
}

impl Deployment {
    pub fn build_monograph_config(
        &self,
        set_ip_list: bool,
        install_dir: String,
    ) -> anyhow::Result<Ini> {
        let mut mysql_ini = Ini::new();
        if let Some(cassandra) = self.storage_service.cassandra.as_ref() {
            mysql_ini
                .load(config_template(MONOGRAPH_CONF_TEMPLATE)?.as_path())
                .unwrap();

            let cassandra_hosts = cassandra.host.join(",");
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_cass_hosts",
                Some(cassandra_hosts),
            );
        } else {
            mysql_ini
                .load(config_template(MONOGRAPH_CONF_DYNAMO_TEMPLATE)?.as_path())
                .unwrap();

            let dynamodb = self.storage_service.dynamodb.as_ref().unwrap();
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_aws_access_key_id",
                Some(dynamodb.clone().access_key_id),
            );
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_aws_secret_key",
                Some(dynamodb.clone().secret_key),
            );
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_dynamodb_region",
                Some(dynamodb.clone().region),
            );
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_dynamodb_endpoint",
                Some(dynamodb.clone().endpoint),
            );
        }

        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "datadir",
            Some(format!("{install_dir}/datafarm")),
        );
        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "lc_messages_dir",
            Some(format!("{install_dir}/monographdb-release/install/share")),
        );
        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "plugin_dir",
            Some(format!(
                "{install_dir}/monographdb-release/install/lib/plugin",
            )),
        );
        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "port",
            Some(self.port.mysql_port.to_string()),
        );

        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "socket",
            Some(format!("/tmp/mysql{}.sock", self.port.mysql_port)),
        );

        let use_port = self.port.monograph_port.start;
        let monograph_hosts = &self.mono_service.host;
        if set_ip_list {
            let ip_list = monograph_hosts
                .iter()
                .map(|host| format!("{}:{}", host.clone(), use_port))
                .join(",");
            mysql_ini.set(CONFIG_MARIADB_SECTION, "monograph_ip_list", Some(ip_list));
        } else {
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_ip_list",
                Some(format!("{}:{}", "127.0.0.1", use_port)),
            );
        }
        Ok(mysql_ini.clone())
    }

    pub fn gen_monograph_config(
        &self,
        db_host: Option<String>,
        install_dir: String,
    ) -> anyhow::Result<PathBuf> {
        let port = self.port.monograph_port.start;
        let set_ip_list = db_host.is_some();
        let my_ini_rs = self.build_monograph_config(set_ip_list, install_dir);

        let host_and_file_tuple = if let Some(host) = db_host {
            (host.clone(), host)
        } else {
            ("127.0.0.1".to_string(), "local".to_string())
        };
        let db_config_location = download_dir().join(format!("my_{}.cnf", host_and_file_tuple.1));
        if let Ok(mut my_ini) = my_ini_rs {
            my_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_local_ip",
                Some(format!("{}:{}", host_and_file_tuple.0, port)),
            );

            if let Err(err) = my_ini.write(db_config_location.clone()) {
                Err(anyhow!(err))
            } else {
                Ok(db_config_location)
            }
        } else {
            Err(my_ini_rs.err().unwrap())
        }
    }
}
