use super::Step;
use crate::cli::task::download_task::DownloadTask;
use crate::cli::task::eloq_store_data_clean_task::EloqStoreDataCleanTask;
use crate::cli::task::exec_custom_cmd::ExecCustomCommand;
use crate::cli::task::failover_op_task::FailoverOpTask;
use crate::cli::task::group::Config;
use crate::cli::task::monograph_log_ctl_task::MonographLogCtlTask;
use crate::cli::task::monograph_log_probe_task::MonographLogProbeTask;
use crate::cli::task::monograph_tx_ctl_task::{MonographTxCtlTask, ServerType};
use crate::cli::task::redis_op_task::{ClusterNodes, RedisOpTask};
use crate::cli::task::task_base::{TaskExecutionContext, TaskHost, TaskId, TaskInstance};
use crate::cli::task::unpack_file_task::UnpackFileTask;
use crate::cli::task::upload::upload_task_builder::{upload_tasks, UploadTaskBuilderType};
use crate::cli::SubCommand;
use crate::config::config_base::DeployConfig;
use crate::config::storage_service_config::DataStoreServiceBackend;
use crate::config::DeploymentPackage;
use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use tokio::sync::watch;
use tracing::info;

// ── Context ───────────────────────────────────────────────────────────────────

/// All configuration extracted upfront from CLI args + deploy config.
/// Each Step reads this to construct its TaskExecutionContext.
#[derive(Clone)]
pub struct UpgradeContext {
    pub config: Config,
    pub deploy: DeployConfig,
    pub cluster: String,
    pub redis_password: Option<String>,
    pub force: bool,
}

impl UpgradeContext {
    pub fn new(cmd_arg: &SubCommand, config: Config) -> Self {
        let deploy = match &config {
            Config::Cluster(c) => c.clone(),
            _ => unreachable!(),
        };
        let (redis_password, force) = match cmd_arg {
            SubCommand::Update {
                password, force, ..
            } => (deploy.redis_password(password.clone()), *force),
            SubCommand::UpdateConf { password, .. } => {
                (deploy.redis_password(password.clone()), false)
            }
            _ => (None, false),
        };
        Self {
            cluster: deploy.deployment.cluster_name.clone(),
            config,
            deploy,
            redis_password,
            force,
        }
    }

    pub fn has_standby(&self) -> bool {
        self.deploy
            .deployment
            .tx_service
            .standby_host_ports
            .is_some()
    }

    pub fn has_voter(&self) -> bool {
        self.deploy.deployment.tx_service.voter_host_ports.is_some()
    }

    pub fn has_log(&self) -> bool {
        self.deploy.deployment.log_service.is_some()
    }

    pub fn tx_host_ports(&self) -> Vec<String> {
        self.deploy
            .get_host_port_list(DeploymentPackage::MonographTx)
    }

    pub fn standby_host_ports(&self) -> Vec<String> {
        self.deploy
            .get_host_port_list(DeploymentPackage::MonographStandby)
    }
}

// ── Helper: build a round of topo→failover→stop ─────────────────────────────

fn build_round(
    round_label: &str,
    nodes_to_failover: &[String],
    nodes_to_stop: &[String],
    all_topology_nodes: &[String],
    ctx: &UpgradeContext,
) -> anyhow::Result<TaskExecutionContext> {
    let mut barrier = vec![];
    let mut executable = IndexMap::new();

    let topo_task_id = TaskId {
        cmd: "topology".to_string(),
        task: format!("check-topology-{round_label}"),
        host: "_local".to_string(),
    };
    let (topo_tx, failover_rx) = watch::channel::<ClusterNodes>(ClusterNodes {
        masters: Vec::new(),
        replicas: Vec::new(),
    });
    let stop_rx = failover_rx.clone();

    executable.insert(
        topo_task_id.clone(),
        TaskInstance {
            task_input: HashMap::default(),
            task: Box::new(RedisOpTask::new(
                topo_task_id,
                all_topology_nodes.to_vec(),
                "cluster topology".to_string(),
                topo_tx,
                ctx.redis_password.clone(),
                true,
            )),
            task_host: TaskHost::Local,
        },
    );
    barrier.push(1);

    for node_addr in nodes_to_failover {
        if let Some((host, port_str)) = node_addr.split_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                let fid = TaskId {
                    cmd: "failover".to_string(),
                    task: format!("failover-check-{round_label}-{port_str}"),
                    host: host.to_string(),
                };
                executable.insert(
                    fid.clone(),
                    TaskInstance {
                        task_input: HashMap::default(),
                        task: Box::new(FailoverOpTask::new(
                            fid,
                            host.to_string(),
                            port,
                            String::new(),
                            0u16,
                            failover_rx.clone(),
                            ctx.redis_password.clone(),
                        )),
                        task_host: TaskHost::Local,
                    },
                );
            }
        }
    }
    barrier.push(nodes_to_failover.len());

    let stop_tasks = MonographTxCtlTask::from_config_with_channel(
        SubCommand::Stop {
            cluster: ctx.cluster.clone(),
            tx: Some(true),
            log: true,
            store: false,
            monitor: false,
            force: ctx.force,
            all: false,
            password: ctx.redis_password.clone(),
            nodes: nodes_to_stop.to_vec(),
        },
        &ctx.deploy,
        ServerType::Node,
        Some(stop_rx),
    )?;
    barrier.push(stop_tasks.len());
    executable.extend(stop_tasks);

    Ok(TaskExecutionContext {
        task_group: format!("rolling-restart-{round_label}"),
        barrier: Some(barrier),
        executable,
    })
}

// ── Concrete Steps ──────────────────────────────────────────────────────────

pub struct DownloadAndUpload {
    ctx: UpgradeContext,
}

impl DownloadAndUpload {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for DownloadAndUpload {
    fn name(&self) -> &str {
        "DownloadAndUpload"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        let mut downloads = vec![];
        let mut upload_img = IndexMap::new();

        downloads.push(self.ctx.deploy.deployment.tx_image().to_owned());
        if let Some(img) = self.ctx.deploy.deployment.log_image() {
            downloads.push(img.to_owned());
        }
        upload_img.extend(upload_tasks(
            UploadTaskBuilderType::EloqImage,
            &self.ctx.config,
        ));

        let download_task = DownloadTask::instances(DownloadTask::from_urls(downloads));
        let barrier = vec![download_task.len(), upload_img.len()];
        let mut executable = IndexMap::new();
        executable.extend(download_task);
        executable.extend(upload_img);

        Ok(TaskExecutionContext {
            task_group: "download-and-upload".to_string(),
            barrier: if barrier.iter().all(|&n| n == 0) {
                None
            } else {
                Some(barrier)
            },
            executable,
        })
    }
}

pub struct StopTxNodes {
    ctx: UpgradeContext,
}

impl StopTxNodes {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StopTxNodes {
    fn name(&self) -> &str {
        "StopTxNodes"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_standby() {
            let stop_tx = MonographTxCtlTask::from_config(
                SubCommand::Stop {
                    cluster: self.ctx.cluster.clone(),
                    tx: Some(true),
                    log: true,
                    store: false,
                    monitor: false,
                    force: self.ctx.force,
                    all: false,
                    password: self.ctx.redis_password.clone(),
                    nodes: Vec::new(),
                },
                &self.ctx.deploy,
                ServerType::Tx,
            );
            let len = stop_tx.len();
            return Ok(TaskExecutionContext {
                task_group: "stop-tx-nodes".to_string(),
                barrier: if len > 0 { Some(vec![len]) } else { None },
                executable: stop_tx,
            });
        }

        // Has standby: failover masters → stop them
        let tx_host_ports = self.ctx.tx_host_ports();
        let standby_host_ports = self.ctx.standby_host_ports();
        let mut all_nodes = tx_host_ports.clone();
        all_nodes.extend(standby_host_ports);

        build_round(
            "round1",
            &tx_host_ports,
            &tx_host_ports,
            &all_nodes,
            &self.ctx,
        )
    }
}

pub struct StopLog {
    ctx: UpgradeContext,
}

impl StopLog {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StopLog {
    fn name(&self) -> &str {
        "StopLog"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_log() {
            return Ok(TaskExecutionContext::dummy());
        }
        let stop_log = MonographLogCtlTask::from_config(
            SubCommand::Stop {
                cluster: self.ctx.cluster.clone(),
                tx: Some(true),
                log: true,
                store: false,
                monitor: false,
                force: self.ctx.force,
                all: false,
                password: self.ctx.redis_password.clone(),
                nodes: Vec::new(),
            },
            &self.ctx.deploy,
        );
        let len = stop_log.len();
        Ok(TaskExecutionContext {
            task_group: "stop-log".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: stop_log,
        })
    }
}

pub struct UnpackTxLog {
    ctx: UpgradeContext,
}

impl UnpackTxLog {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for UnpackTxLog {
    fn name(&self) -> &str {
        "UnpackTxLog"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        let unpack = UnpackFileTask::unpack_tx_and_log_nodes(&self.ctx.deploy);
        let len = unpack.len();
        Ok(TaskExecutionContext {
            task_group: "unpack-tx-log".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: unpack,
        })
    }
}

pub struct CleanEloqStoreData {
    ctx: UpgradeContext,
}

impl CleanEloqStoreData {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for CleanEloqStoreData {
    fn name(&self) -> &str {
        "CleanEloqStoreData"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        let deployment = &self.ctx.deploy.deployment;
        let start_cmd = SubCommand::Start {
            cluster: self.ctx.cluster.clone(),
            nodes: Vec::new(),
        };

        if let Some(storage_service) = &deployment.storage_service {
            if let Some(dss) = &storage_service.eloqdss {
                match dss.backend_config() {
                    DataStoreServiceBackend::EloqStore(eloq_store_config) => {
                        if eloq_store_config.is_cloud_mode() {
                            let should_skip_cleanup = eloq_store_config
                                .get_cloud_config()
                                .and_then(|cc| cc.eloq_store_reuse_local_files)
                                .unwrap_or(false);
                            if !should_skip_cleanup {
                                let clean_tasks = EloqStoreDataCleanTask::build_tasks(
                                    start_cmd,
                                    &self.ctx.config,
                                    None,
                                );
                                if !clean_tasks.is_empty() {
                                    let len = clean_tasks.len();
                                    return Ok(TaskExecutionContext {
                                        task_group: "clean-eloq-store-data".to_string(),
                                        barrier: Some(vec![len]),
                                        executable: clean_tasks,
                                    });
                                }
                            } else {
                                info!(
                                    "Skipping EloqStore data cleanup (reuse_local_files enabled)"
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(TaskExecutionContext::dummy())
    }
}

pub struct StartLogAndWait {
    ctx: UpgradeContext,
}

impl StartLogAndWait {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StartLogAndWait {
    fn name(&self) -> &str {
        "StartLogAndWait"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_log() {
            return Ok(TaskExecutionContext::dummy());
        }
        let start_cmd = SubCommand::Start {
            cluster: self.ctx.cluster.clone(),
            nodes: Vec::new(),
        };
        let start_log = MonographLogCtlTask::from_config(start_cmd, &self.ctx.deploy);
        let probe = MonographLogProbeTask::from_config(&self.ctx.deploy);

        let mut barrier = vec![];
        let mut executable = IndexMap::new();

        if !start_log.is_empty() {
            barrier.push(start_log.len());
            executable.extend(start_log);
        }
        if !probe.is_empty() {
            barrier.push(probe.len());
            executable.extend(probe);
        }

        Ok(TaskExecutionContext {
            task_group: "start-log-and-wait".to_string(),
            barrier: if barrier.is_empty() {
                None
            } else {
                Some(barrier)
            },
            executable,
        })
    }
}

pub struct StartTx {
    ctx: UpgradeContext,
}

impl StartTx {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StartTx {
    fn name(&self) -> &str {
        "StartTx"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        let start_tx = MonographTxCtlTask::from_config(
            SubCommand::Start {
                cluster: self.ctx.cluster.clone(),
                nodes: Vec::new(),
            },
            &self.ctx.deploy,
            ServerType::Tx,
        );
        let len = start_tx.len();
        Ok(TaskExecutionContext {
            task_group: "start-tx".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: start_tx,
        })
    }
}

pub struct WaitTxReady {
    ctx: UpgradeContext,
}

impl WaitTxReady {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for WaitTxReady {
    fn name(&self) -> &str {
        "WaitTxReady"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        let probe = MonographTxCtlTask::from_config(
            SubCommand::Status {
                cluster: self.ctx.cluster.clone(),
                user: None,
                password: self.ctx.redis_password.clone(),
                wait: Some(120),
                detail: false,
            },
            &self.ctx.deploy,
            ServerType::Tx,
        );
        let len = probe.len();
        Ok(TaskExecutionContext {
            task_group: "wait-tx-ready".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: probe,
        })
    }
}

pub struct FailoverBackAndStopStandby {
    ctx: UpgradeContext,
}

impl FailoverBackAndStopStandby {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for FailoverBackAndStopStandby {
    fn name(&self) -> &str {
        "FailoverBackAndStopStandby"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_standby() {
            return Ok(TaskExecutionContext::dummy());
        }

        let standby_host_ports = self.ctx.standby_host_ports();
        let tx_host_ports = self.ctx.tx_host_ports();
        let mut all_nodes = standby_host_ports.clone();
        all_nodes.extend(tx_host_ports);

        build_round(
            "round2",
            &standby_host_ports,
            &standby_host_ports,
            &all_nodes,
            &self.ctx,
        )
    }
}

pub struct UnpackStandby {
    ctx: UpgradeContext,
}

impl UnpackStandby {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for UnpackStandby {
    fn name(&self) -> &str {
        "UnpackStandby"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_standby() {
            return Ok(TaskExecutionContext::dummy());
        }
        let unpack = UnpackFileTask::unpack_standby_nodes(&self.ctx.deploy);
        let len = unpack.len();
        Ok(TaskExecutionContext {
            task_group: "unpack-standby".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: unpack,
        })
    }
}

pub struct StartStandby {
    ctx: UpgradeContext,
}

impl StartStandby {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StartStandby {
    fn name(&self) -> &str {
        "StartStandby"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_standby() {
            return Ok(TaskExecutionContext::dummy());
        }
        let start = MonographTxCtlTask::from_config(
            SubCommand::Start {
                cluster: self.ctx.cluster.clone(),
                nodes: Vec::new(),
            },
            &self.ctx.deploy,
            ServerType::Standby,
        );
        let len = start.len();
        Ok(TaskExecutionContext {
            task_group: "start-standby".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: start,
        })
    }
}

pub struct StopVoters {
    ctx: UpgradeContext,
}

impl StopVoters {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StopVoters {
    fn name(&self) -> &str {
        "StopVoters"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_voter() {
            return Ok(TaskExecutionContext::dummy());
        }
        let stop = MonographTxCtlTask::from_config(
            SubCommand::Stop {
                cluster: self.ctx.cluster.clone(),
                tx: None,
                log: false,
                store: false,
                monitor: false,
                force: self.ctx.force,
                all: false,
                password: self.ctx.redis_password.clone(),
                nodes: Vec::new(),
            },
            &self.ctx.deploy,
            ServerType::Voter,
        );
        let len = stop.len();
        Ok(TaskExecutionContext {
            task_group: "stop-voters".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: stop,
        })
    }
}

pub struct StartVoters {
    ctx: UpgradeContext,
}

impl StartVoters {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for StartVoters {
    fn name(&self) -> &str {
        "StartVoters"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        if !self.ctx.has_voter() {
            return Ok(TaskExecutionContext::dummy());
        }
        let start = MonographTxCtlTask::from_config(
            SubCommand::Start {
                cluster: self.ctx.cluster.clone(),
                nodes: Vec::new(),
            },
            &self.ctx.deploy,
            ServerType::Voter,
        );
        let len = start.len();
        Ok(TaskExecutionContext {
            task_group: "start-voters".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: start,
        })
    }
}

pub struct VerifyVersion {
    ctx: UpgradeContext,
}

impl VerifyVersion {
    pub fn new(ctx: UpgradeContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl Step for VerifyVersion {
    fn name(&self) -> &str {
        "VerifyVersion"
    }

    async fn build(&self) -> anyhow::Result<TaskExecutionContext> {
        let tasks = ExecCustomCommand::build_task_by_host(
            format!(
                "{}/EloqKV/bin/eloqkv --version",
                self.ctx.deploy.install_dir()
            ),
            &self.ctx.config,
            self.ctx.deploy.deployment.tx_service.merge_hosts(),
            Some("check_eloqkv_version".to_string()),
        );
        let len = tasks.len();
        Ok(TaskExecutionContext {
            task_group: "verify-version".to_string(),
            barrier: if len > 0 { Some(vec![len]) } else { None },
            executable: tasks,
        })
    }
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Build the list of steps for a rolling binary upgrade (`eloqctl update`).
pub fn build_upgrade_steps(ctx: UpgradeContext) -> Vec<Box<dyn Step>> {
    vec![
        Box::new(DownloadAndUpload::new(ctx.clone())),
        Box::new(StopTxNodes::new(ctx.clone())),
        Box::new(StopLog::new(ctx.clone())),
        Box::new(UnpackTxLog::new(ctx.clone())),
        Box::new(CleanEloqStoreData::new(ctx.clone())),
        Box::new(StartLogAndWait::new(ctx.clone())),
        Box::new(StartTx::new(ctx.clone())),
        Box::new(WaitTxReady::new(ctx.clone())),
        Box::new(FailoverBackAndStopStandby::new(ctx.clone())),
        Box::new(UnpackStandby::new(ctx.clone())),
        Box::new(StartStandby::new(ctx.clone())),
        Box::new(StartVoters::new(ctx.clone())),
        Box::new(VerifyVersion::new(ctx.clone())),
    ]
}

/// Build the list of steps for a rolling config restart (`eloqctl update-conf --restart`).
pub fn build_config_restart_steps(ctx: UpgradeContext) -> Vec<Box<dyn Step>> {
    vec![
        Box::new(StopTxNodes::new(ctx.clone())),
        Box::new(StartTx::new(ctx.clone())),
        Box::new(WaitTxReady::new(ctx.clone())),
        Box::new(FailoverBackAndStopStandby::new(ctx.clone())),
        Box::new(StartStandby::new(ctx.clone())),
        Box::new(StopVoters::new(ctx.clone())),
        Box::new(StartVoters::new(ctx.clone())),
    ]
}
