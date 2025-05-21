use crate::cli::task::task_base::{ExecutionValue, TaskArgValue, TaskHost};
use crate::cli::{CMD, CMD_OUTPUT, CMD_STATUS};
use anyhow::bail;
use async_trait::async_trait;
use futures::AsyncWriteExt;
use itertools::Itertools;
use russh::*;
use russh_keys::*;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::vec;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct SSHClient {}

#[async_trait]
impl client::Handler for SSHClient {
    type Error = russh::Error;

    async fn check_server_key(
        self,
        _server_public_key: &key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        Ok((self, true))
    }
}

#[derive(Clone, Debug)]
pub enum SSHCommandOption {
    CollectOutput,
    None,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct SSHSession {
    session: Arc<Mutex<client::Handle<SSHClient>>>,
    user: String,
    host: String,
    port: usize,
}

impl SSHSession {
    pub async fn from_task_host(host: TaskHost, key_path: String) -> anyhow::Result<Self> {
        match host {
            TaskHost::Remote {
                user,
                port: ssh_port,
                host,
            } => {
                info!("ssh connect key {key_path}");
                SSHSession::connect(key_path, user.as_str(), &host, ssh_port).await
            }
            _ => {
                unreachable!()
            }
        }
    }

    pub async fn connect<P: AsRef<Path>>(
        key_path: P,
        user: &str,
        host: &str,
        port: usize,
    ) -> anyhow::Result<Self> {
        // Connection configuration
        const MAX_RETRIES: usize = 3;
        const CONNECTION_TIMEOUT_SECS: u64 = 10;
        const RETRY_DELAY_SECS: u64 = 2;
        const OVERALL_TIMEOUT_SECS: u64 = 30;

        // SSH client configuration
        let ssh_config = Arc::new(client::Config {
            keepalive_interval: Some(Duration::from_secs(3)),
            ..Default::default()
        });

        // Wrap the entire connection process in an overall timeout
        match timeout(Duration::from_secs(OVERALL_TIMEOUT_SECS), async {
            // Step 1: Resolve address
            let ssh_addr = Self::resolve_address(host, port)?;

            // Step 2: Load SSH key
            let key_pair = match load_secret_key(&key_path, None) {
                Ok(key) => key,
                Err(e) => bail!(
                    "Failed to load SSH key from {}: {}",
                    key_path.as_ref().display(),
                    e
                ),
            };

            // Step 3: Attempt connection with retries
            Self::attempt_connection(
                ssh_config,
                &ssh_addr,
                user,
                host,
                port,
                key_pair,
                MAX_RETRIES,
                CONNECTION_TIMEOUT_SECS,
                RETRY_DELAY_SECS,
            )
            .await
        })
        .await
        {
            Ok(result) => result,
            Err(_) => bail!(
                "Overall SSH connection process timed out after {} seconds",
                OVERALL_TIMEOUT_SECS
            ),
        }
    }

    // Helper function to resolve host address
    fn resolve_address(host: &str, port: usize) -> anyhow::Result<std::net::SocketAddr> {
        let socket_str = format!("{host}:{port}");
        let mut addrs = match socket_str.to_socket_addrs() {
            Ok(addrs) => addrs,
            Err(e) => {
                bail!("Failed to resolve address [{socket_str}]: {e}. Please check deployment.yaml")
            }
        };

        // Find first valid IPv4 address
        let addr = addrs
            .find(|addr| addr.is_ipv4() && !addr.to_string().contains("0.0.0.0"))
            .ok_or_else(|| anyhow::anyhow!("No valid IPv4 address found for [{host}:{port}]"))?;

        Ok(addr)
    }

    // Helper function to attempt connection with retries
    async fn attempt_connection(
        ssh_config: Arc<client::Config>,
        ssh_addr: &std::net::SocketAddr,
        user: &str,
        host: &str,
        port: usize,
        key_pair: key::KeyPair,
        max_retries: usize,
        connection_timeout_secs: u64,
        retry_delay_secs: u64,
    ) -> anyhow::Result<Self> {
        let ssh_client = SSHClient {};
        let mut last_error = None;

        for attempt in 1..=max_retries {
            info!("SSH connection attempt {} to {}:{}", attempt, host, port);

            // Step 3a: Establish connection
            let session = match Self::establish_connection(
                ssh_config.clone(),
                ssh_addr,
                &ssh_client,
                connection_timeout_secs,
            )
            .await
            {
                Ok(session) => session,
                Err(e) => {
                    warn!("{}", e);
                    last_error = Some(e);

                    // If this wasn't the last attempt, wait before retrying
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_secs(retry_delay_secs)).await;
                    }
                    continue;
                }
            };

            // Step 3b: Authenticate
            match Self::authenticate_session(
                session,
                user,
                key_pair.clone(),
                ssh_addr,
                connection_timeout_secs,
            )
            .await
            {
                Ok(session) => {
                    info!("SSH connection established to {}@{}", user, host);
                    return Ok(Self {
                        session: Arc::new(Mutex::new(session)),
                        user: user.to_string(),
                        host: host.to_string(),
                        port,
                    });
                }
                Err(e) => {
                    warn!("{}", e);
                    let error_str = e.to_string();
                    last_error = Some(e);
                    // If auth was rejected, no point retrying
                    if error_str.contains("SSH authentication failed") {
                        break;
                    }

                    // If this wasn't the last attempt, wait before retrying
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_secs(retry_delay_secs)).await;
                    }
                }
            }
        }

        // If we got here, all retries failed
        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "Failed to connect to {user}@{host}:{port} after {max_retries} attempts"
            )
        }))
    }

    // Helper function to establish connection
    async fn establish_connection(
        ssh_config: Arc<client::Config>,
        ssh_addr: &std::net::SocketAddr,
        ssh_client: &SSHClient,
        timeout_secs: u64,
    ) -> anyhow::Result<client::Handle<SSHClient>> {
        match timeout(
            Duration::from_secs(timeout_secs),
            client::connect(ssh_config, ssh_addr, ssh_client.clone()),
        )
        .await
        {
            Ok(Ok(session)) => Ok(session),
            Ok(Err(e)) => bail!("SSH connection error: {}", e),
            Err(_) => bail!("SSH connection timed out after {} seconds", timeout_secs),
        }
    }

    // Helper function to authenticate session
    async fn authenticate_session(
        mut session: client::Handle<SSHClient>,
        user: &str,
        key_pair: key::KeyPair,
        ssh_addr: &std::net::SocketAddr,
        timeout_secs: u64,
    ) -> anyhow::Result<client::Handle<SSHClient>> {
        match timeout(
            Duration::from_secs(timeout_secs),
            session.authenticate_publickey(user, Arc::new(key_pair)),
        )
        .await
        {
            Ok(Ok(true)) => Ok(session),
            Ok(Ok(false)) => bail!("SSH authentication failed for {user}@{ssh_addr}"),
            Ok(Err(e)) => bail!("SSH authentication error: {}", e),
            Err(_) => bail!(
                "SSH authentication timed out after {} seconds",
                timeout_secs
            ),
        }
    }

    pub fn ssh_conn_info(&self) -> (String, String) {
        (self.host.clone(), self.user.clone())
    }

    pub async fn command(
        &self,
        command: &str,
        cmd_option: SSHCommandOption,
    ) -> anyhow::Result<ExecutionValue> {
        let session = self.session.lock().await;
        let mut channel = session.channel_open_session().await?;
        channel.exec(true, command).await?;
        let mut output = Vec::new();
        let mut status_code = 0_i32;
        while let Some(chanel_msg) = channel.wait().await {
            match chanel_msg {
                ChannelMsg::Data { ref data } => {
                    if let SSHCommandOption::CollectOutput = cmd_option {
                        output.write_all(data).await?;
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    status_code = exit_status as i32;
                }
                ChannelMsg::Failure => {
                    error!("ssh channel receive failure msg.");
                }
                _ => {}
            }
        }
        let output_str = String::from_utf8_lossy(&output).into_owned();
        let cmd_res = HashMap::from([
            (CMD.to_string(), TaskArgValue::Str(command.to_string())),
            (CMD_STATUS.to_string(), TaskArgValue::Number(status_code)),
            (CMD_OUTPUT.to_string(), TaskArgValue::Str(output_str)),
        ]);
        if status_code != 0 {
            let conn_info = (self.host.clone(), self.port);
            warn!("SSHSession Failed execute command. ssh_info={conn_info:?}, {cmd_res:#?}] ");
        }
        channel.close().await?;
        Ok(cmd_res)
    }

    pub async fn execute(&self, command: &str) -> anyhow::Result<(i32, String)> {
        let ret = self
            .command(command, SSHCommandOption::CollectOutput)
            .await?;
        let code = TaskArgValue::into_inner_value::<i32>(ret.get(CMD_STATUS).unwrap().clone());
        let output = TaskArgValue::into_inner_value::<String>(ret.get(CMD_OUTPUT).unwrap().clone())
            .trim()
            .to_owned();
        Ok((code, output))
    }

    pub async fn used_tcp_ports(&self) -> anyhow::Result<Vec<u16>> {
        let host = &self.host;
        let cmd = "ss -tnl | awk 'NR>1 {print $4}' | awk -F':' '{print $NF}'";
        let (code, output) = self.execute(cmd).await?;
        if code != 0 {
            bail!("can't read used port at {host}, return {code}")
        }
        info!(
            "socket {host}:{} is already used",
            output.replace('\n', ",")
        );
        let used = output
            .lines()
            .filter_map(|s| match s.parse::<u16>() {
                Ok(port) => Some(port),
                Err(err) => {
                    warn!("can't parse port '{s}': {err}");
                    None
                }
            })
            .unique()
            .collect();
        Ok(used)
    }

    pub async fn parallel(
        key: String,
        user: &str,
        port: usize,
        hosts: Vec<String>,
        content: &str,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let mut joins = vec![];
        hosts.into_iter().for_each(|host| {
            let task_host = TaskHost::Remote {
                user: user.to_owned(),
                port,
                host: host.clone(),
            };
            let key_path = key.clone();
            let c = content.to_owned();
            let join = tokio::task::spawn(async move {
                let sess = Self::from_task_host(task_host, key_path).await?;
                let output = sess.execute(&c).await?.1;
                sess.close().await?;
                anyhow::Ok((host, output))
            });
            joins.push(join);
        });
        let mut all_out = vec![];
        for join_res in futures::future::join_all(joins).await {
            let out = join_res??;
            all_out.push(out);
        }
        Ok(all_out)
    }

    pub async fn close(&self) -> anyhow::Result<()> {
        let session = self.session.lock().await;
        session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}
