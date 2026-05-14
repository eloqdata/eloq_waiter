use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Connection {
    pub username: String,
    pub auth_type: String,
    pub auth: Auth,
    pub port: Option<u16>,
    pub ssh_endpoints: Option<HashMap<String, SshEndpoint>>,
    pub service_endpoints: Option<HashMap<String, ServiceEndpoint>>,
}

impl Connection {
    pub fn ssh_port(&self) -> u16 {
        self.port.unwrap_or(22)
    }

    pub fn ssh_auth_key(&self) -> Option<String> {
        self.auth.clone().keypair
    }

    pub fn ssh_endpoint(&self, host: &str) -> (String, u16) {
        self.ssh_endpoints
            .as_ref()
            .and_then(|endpoints| endpoints.get(host))
            .map(|endpoint| {
                (
                    endpoint.host.clone(),
                    endpoint.port.unwrap_or_else(|| self.ssh_port()),
                )
            })
            .unwrap_or_else(|| (host.to_string(), self.ssh_port()))
    }

    pub fn service_endpoint(&self, host: &str, port: u16) -> (String, u16) {
        resolve_service_endpoint(self.service_endpoints.as_ref(), host, port)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SshEndpoint {
    pub host: String,
    pub port: Option<u16>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ServiceEndpoint {
    pub host: String,
    pub port: Option<u16>,
}

pub fn resolve_service_endpoint(
    endpoints: Option<&HashMap<String, ServiceEndpoint>>,
    host: &str,
    port: u16,
) -> (String, u16) {
    endpoints
        .and_then(|endpoints| {
            endpoints
                .get(&format!("{host}:{port}"))
                .or_else(|| endpoints.get(host))
        })
        .map(|endpoint| (endpoint.host.clone(), endpoint.port.unwrap_or(port)))
        .unwrap_or_else(|| (host.to_string(), port))
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Auth {
    pub password: Option<String>,
    pub keypair: Option<String>,
}

impl Auth {
    pub fn check_keypair(&self) -> Result<()> {
        if let Some(sshkey) = &self.keypair {
            if !Path::new(sshkey).exists() {
                bail!("ssh key {sshkey} not exist");
            }
        }
        Ok(())
    }
}
