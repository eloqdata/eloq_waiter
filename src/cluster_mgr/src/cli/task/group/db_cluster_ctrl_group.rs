use crate::cli::task::cassandra_ctl_task::CassandraCtlTask;
use crate::cli::task::codis_task::{self, CodisTask};
use crate::cli::task::group::{CtrlDBTaskGroup, TaskGroup};
use crate::cli::task::monograph_log_ctl_task::MonographLogCtlTask;
use crate::cli::task::monograph_log_probe_task::MonographLogProbeTask;
use crate::cli::task::monograph_tx_ctl_task::MonographTxCtlTask;
use crate::cli::task::task_base::{TaskExecutionContext, TaskId, TaskInstance};
use crate::cli::SubCommand;
use crate::config::config_base::DeployConfig;
use anyhow::Result;
use indexmap::IndexMap;

#[async_trait::async_trait]
impl TaskGroup for CtrlDBTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: SubCommand,
        config: DeployConfig,
    ) -> Result<TaskExecutionContext> {
        let cmd_str = cmd_arg.as_ref().to_owned();
        let (barrier, executable) = match cmd_arg {
            SubCommand::Restart { cluster } => {
                let stop_cmd = SubCommand::Stop {
                    cluster: cluster.clone(),
                    force: false,
                    all: false,
                };
                let (mut barrier, mut executable) = self.stop_tasks(stop_cmd, &config);
                let start_cmd = SubCommand::Start { cluster };
                let (b, exe) = self.start_tasks(start_cmd, &config);
                barrier.extend(b);
                executable.extend(exe);
                (barrier, executable)
            }
            SubCommand::Start { .. } => self.start_tasks(cmd_arg, &config),
            SubCommand::Stop { .. } => self.stop_tasks(cmd_arg, &config),
            SubCommand::Status { .. } => {
                let tasks = self.status_tasks(cmd_arg, &config);
                (vec![tasks.len()], tasks)
            }
            _ => unreachable!(),
        };

        Ok(TaskExecutionContext {
            task_group: format!("cluster-control-{cmd_str}"),
            barrier: Some(barrier),
            executable,
        })
    }
}

impl CtrlDBTaskGroup {
    fn stop_tasks(
        &self,
        stop_cmd: SubCommand,
        config: &DeployConfig,
    ) -> (Vec<usize>, IndexMap<TaskId, TaskInstance>) {
        let stop_cass = match stop_cmd {
            SubCommand::Stop { all, .. } => all,
            _ => unreachable!(),
        };
        let deployment = &config.deployment;
        let mut barrier = vec![];
        let mut executable = IndexMap::new();

        if deployment.codis.is_some() {
            let codis_tasks = CodisTask::from_config(&config, codis_task::Operation::Stop);
            if !codis_tasks.is_empty() {
                barrier.push(codis_tasks.len());
                executable.extend(codis_tasks);
            }
        }

        // stop order: tx-server -> log-server -> cassandra
        let stop_tx = MonographTxCtlTask::from_config(stop_cmd.clone(), &config);
        barrier.push(stop_tx.len());
        executable.extend(stop_tx);
        if deployment.log_service.is_some() {
            let stop_log = MonographLogCtlTask::from_config(stop_cmd.clone(), &config);
            barrier.push(stop_log.len());
            executable.extend(stop_log);
        }
        if stop_cass && deployment.storage_service.inner_cass().is_some() {
            let tasks = CassandraCtlTask::from_config(stop_cmd, &config);
            barrier.push(tasks.len());
            executable.extend(tasks);
        }
        (barrier, executable)
    }

    fn start_tasks(
        &self,
        start_cmd: SubCommand,
        config: &DeployConfig,
    ) -> (Vec<usize>, IndexMap<TaskId, TaskInstance>) {
        let deployment = &config.deployment;
        let mut barrier = vec![];
        let mut executable = IndexMap::new();
        // start order: cassandra -> log-server -> tx-server
        if deployment.storage_service.inner_cass().is_some() {
            let tasks = CassandraCtlTask::from_config(start_cmd.clone(), &config);
            let ba = CassandraCtlTask::start_barrier(tasks.len());
            barrier.extend(ba);
            executable.extend(tasks);
        }
        if deployment.log_service.is_some() {
            let start_log = MonographLogCtlTask::from_config(start_cmd.clone(), &config);
            barrier.push(start_log.len());
            executable.extend(start_log);
            let probe = MonographLogProbeTask::from_config(&config);
            barrier.push(probe.len());
            executable.extend(probe);
        }
        let start_tx = MonographTxCtlTask::from_config(start_cmd, &config);
        barrier.push(start_tx.len());
        executable.extend(start_tx);

        if deployment.codis.is_some() {
            let codis_tasks = CodisTask::from_config(&config, codis_task::Operation::Start);
            if !codis_tasks.is_empty() {
                // start dashboard firstly, and then start all proxy servers
                barrier.push(1);
                barrier.push(codis_tasks.len() - 1);
                executable.extend(codis_tasks);
            }
        }

        (barrier, executable)
    }

    fn status_tasks(
        &self,
        cmd: SubCommand,
        config: &DeployConfig,
    ) -> IndexMap<TaskId, TaskInstance> {
        let deployment = &config.deployment;
        let mut executable = IndexMap::new();
        if deployment.log_service.is_some() {
            let tasks = MonographLogCtlTask::from_config(cmd.clone(), &config);
            executable.extend(tasks);
        }
        let start_tx = MonographTxCtlTask::from_config(cmd, &config);
        executable.extend(start_tx);
        if deployment.codis.is_some() {
            //TODO
        }
        executable
    }
}
