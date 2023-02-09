use crate::config::config_base::{
    GRAFANA_FILE_KEY, MYSQL_EXPORTER_FILE_KEY, NODE_EXPORTER_FILE_KEY, PROMETHEUS_FILE_KEY,
};
use crate::config::DownloadUrl;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[macro_export]
macro_rules! monitor_components {
    ($component_name:ident) => {
        #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
        pub struct $component_name {
            pub download_url: String,
            pub port: u16,
            pub host: String,
        }
    };
}

monitor_components!(Prometheus);
monitor_components!(Grafana);

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
pub struct StorageService {
    pub cassandra: Option<Cassandra>,
    pub dynamodb: Option<Dynamodb>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Cassandra {
    pub host: Vec<String>,
    pub download_url: String,
    pub storage_cluster: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Dynamodb {
    pub access_key_id: String,
    pub secret_key: String,
    pub region: String,
    pub endpoint: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Monitor {
    pub prometheus: Prometheus,
    pub grafana: Grafana,
    pub node_exporter: String,
    pub mysql_exporter: String,
}

impl Monitor {
    pub fn monitor_download_links(&self) -> anyhow::Result<Vec<DownloadUrl>> {
        let download_links = self.monitor_download_links_as_amp()?;
        Ok(download_links.into_values().collect_vec())
    }

    pub fn monitor_download_links_as_amp(&self) -> anyhow::Result<HashMap<String, DownloadUrl>> {
        Ok(HashMap::from([
            (
                PROMETHEUS_FILE_KEY.to_string(),
                DownloadUrl::from_url_str(self.prometheus.download_url.as_str())?,
            ),
            (
                GRAFANA_FILE_KEY.to_string(),
                DownloadUrl::from_url_str(self.grafana.download_url.as_str())?,
            ),
            (
                NODE_EXPORTER_FILE_KEY.to_string(),
                DownloadUrl::from_url_str(self.node_exporter.as_str())?,
            ),
            (
                MYSQL_EXPORTER_FILE_KEY.to_string(),
                DownloadUrl::from_url_str(self.mysql_exporter.as_str())?,
            ),
        ]))
    }
}
