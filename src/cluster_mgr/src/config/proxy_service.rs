use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ProxyService {
    pub proxy_name: String,
    pub bin_download_url: Option<String>,
    pub proxy_addrs: Vec<String>,
    pub web_service_ports: Vec<String>,
    pub install_dir: String,
    pub eloqkv_cluster_addr: Vec<String>,
    pub eloqkv_cluster_token: Vec<String>,
    pub eloqkv_cluster_password: Vec<String>,
}

impl ProxyService {
    pub fn install_dir(&self) -> String {
        format!("{}/proxy-service", &self.install_dir)
    }

    pub fn proxy_bin(&self) -> String {
        format!("{}/eloqkv-proxy", &self.install_dir())
    }

    pub fn proxy_conf_path(&self) -> String {
        format!("{}/eloqproxy.ini", &self.install_dir())
    }
}
