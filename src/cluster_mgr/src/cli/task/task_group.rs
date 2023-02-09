use crate::cli::task::cassandra_ctl_task::CassandraCtlTask;
use crate::cli::task::download_task::DownloadFromRemoteTask;
use crate::cli::task::exec_custom_cmd::ExecCustomCommand;
use crate::cli::task::local_copy_task::LocalCopyTask;
use crate::cli::task::monograph_ctl_task::MonographCtlTask;
use crate::cli::task::monograph_install_task::MonographInstall;
use crate::cli::task::runtime_deps_install::RuntimeDepsInstallation;
use crate::cli::task::task_base::{TaskExecutionContext, TaskHost, TaskId, TaskInstance};
use crate::cli::task::unpack_file_task::UnpackFileTask;
use crate::cli::task::upload_task::UploadTask;
use crate::cli::CommandArgs;
use crate::config::config_base::DeploymentConfig;
use crate::config::{DeploymentService, StorageProvider};
use crate::state::state_mgr::STATE_MGR;
use crate::state::task_status_operation::TaskStatusEntity;
use dyn_clone::DynClone;
use indexmap::IndexMap;
use itertools::Itertools;
use std::collections::HashMap;
use std::sync::LazyLock;
use tracing::info;

/// `TaskGroup` base on different business logic, multiple tasks are organized into task groups,
/// and barriers are inserted between task lists according to dependencies.
#[async_trait::async_trait]
pub trait TaskGroup: Send + Sync + DynClone {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext>;
}

dyn_clone::clone_trait_object!(TaskGroup);

#[macro_export]
macro_rules! task_group_boxed {
    ($({$task_group:ident}),*) => {
        $(
        #[derive(Clone)]
        struct $task_group;

        impl $task_group {
            fn boxed() -> Box<dyn TaskGroup> {
                Box::new(Self {})
            }
        }
        )*
    };
}

task_group_boxed! {
    {DeploymentTaskGroup},
    {InstallDBTaskGroup},
    {CtrlDBTaskGroup},
    {CustomCmdTaskGroup},
    {InstallRuntimeDepsTaskGroup}
}

pub static TASK_GROUP: LazyLock<HashMap<String, Box<dyn TaskGroup>>> = LazyLock::new(|| {
    HashMap::from([
        ("deploy".to_string(), DeploymentTaskGroup::boxed()),
        ("install".to_string(), InstallDBTaskGroup::boxed()),
        ("start".to_string(), CtrlDBTaskGroup::boxed()),
        ("stop".to_string(), CtrlDBTaskGroup::boxed()),
        ("restart".to_string(), CtrlDBTaskGroup::boxed()),
        ("status".to_string(), CtrlDBTaskGroup::boxed()),
        ("exec_cmd".to_string(), CustomCmdTaskGroup::boxed()),
        ("run-deps".to_string(), InstallRuntimeDepsTaskGroup::boxed()),
    ])
});

impl DeploymentTaskGroup {
    fn skip_success_task_execution(
        task_instances: &IndexMap<TaskId, TaskInstance>,
        success_task_entity: &[TaskStatusEntity],
    ) -> IndexMap<TaskId, TaskInstance> {
        if success_task_entity.is_empty() {
            task_instances.clone()
        } else {
            success_task_entity
                .iter()
                .map(|task_status| {
                    let task_id_string = &task_status.task;
                    TaskId::from_json_string(task_id_string.clone())
                })
                .filter(|task_id| !task_instances.contains_key(task_id))
                .map(|task_id| {
                    (
                        task_id.clone(),
                        task_instances.get(&task_id).unwrap().clone(),
                    )
                })
                .collect::<IndexMap<TaskId, TaskInstance>>()
        }
    }
}

#[async_trait::async_trait]
impl TaskGroup for DeploymentTaskGroup {
    async fn tasks(
        &self,
        cmd_args: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let cmd_ref = cmd_args.as_ref().to_string();
        let cluster = &config.deployment.cluster_name;

        let success_task_entity = STATE_MGR
            .load_task_status_from_state(cluster.to_string(), Some(0), Some(vec![cmd_ref.clone()]))
            .await?;

        let download_task = DownloadFromRemoteTask::from_config(&config)?;
        let mut copy_or_download_task_instances = LocalCopyTask::form_config(&config)?;
        copy_or_download_task_instances.extend(download_task.into_iter());

        let upload_task = DeploymentTaskGroup::skip_success_task_execution(
            &UploadTask::from_config(&config)?,
            &success_task_entity,
        );

        let unpack_task = DeploymentTaskGroup::skip_success_task_execution(
            &UnpackFileTask::from_config(&config)?,
            &success_task_entity,
        );

        let barrier = Some(vec![
            copy_or_download_task_instances.len(),
            upload_task.len(),
            unpack_task.len(),
        ]);
        let mut executable = IndexMap::new();
        executable.extend(copy_or_download_task_instances.into_iter());
        executable.extend(upload_task.into_iter());
        executable.extend(unpack_task.into_iter());
        Ok(TaskExecutionContext {
            task_group: cmd_ref,
            barrier,
            executable,
        })
    }
}

//TODO refactor
#[async_trait::async_trait]
impl TaskGroup for InstallDBTaskGroup {
    async fn tasks(
        &self,
        cmd_args: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let monograph_hosts = config.get_host_list(DeploymentService::Monograph);
        let monograph_hosts_len = monograph_hosts.len();
        assert!(monograph_hosts_len >= 1);
        let conn_user = &config.connection.username;
        let ssh_port = config.connection.ssh_port();
        let install_db_host_string = monograph_hosts.first().unwrap();
        let install_db_host = TaskHost::Remote {
            user: conn_user.clone(),
            port: ssh_port as usize,
            hosts: install_db_host_string.clone(),
        };
        info!(
            "InstallDBTaskGroup The list of MonographDB node is: {:?}, install_db_host={:?}",
            monograph_hosts, install_db_host
        );
        let install_cmd = CommandArgs::Install {
            cluster: config.clone().deployment.cluster_name,
        };
        let storage_provider = config.get_monograph_storage()?;

        let mut execution_context_tuple = match storage_provider {
            StorageProvider::Cassandra => {
                let upload_cass_config_task = UploadTask::build_upload_cass_conf_task(&config)?;
                let cassandra_start = CassandraCtlTask::from_config(install_cmd, &config);
                let monograph_install = MonographInstall::from_config(&config, install_db_host);
                let barrier = vec![
                    upload_cass_config_task.len(),
                    cassandra_start.len(),
                    monograph_install.len(),
                ];
                let mut executable = IndexMap::new();
                executable.extend(upload_cass_config_task.into_iter());
                executable.extend(cassandra_start.into_iter());
                executable.extend(monograph_install.into_iter());
                TaskExecutionContext {
                    task_group: cmd_args.as_ref().to_string(),
                    barrier: Some(barrier),
                    executable,
                }
            }
            _ => {
                let monograph_is_multi_node = monograph_hosts.len() > 1;
                let monograph_install = MonographInstall::from_config(&config, install_db_host);
                TaskExecutionContext {
                    task_group: cmd_args.as_ref().to_string(),
                    barrier: if monograph_is_multi_node {
                        Some(vec![monograph_install.len()])
                    } else {
                        None
                    },
                    executable: monograph_install,
                }
            }
        };
        let mut barrier = execution_context_tuple.clone().barrier.unwrap();
        let mut executable = execution_context_tuple.executable;
        if monograph_hosts.len() > 1 {
            let dest_hosts = monograph_hosts[1..=monograph_hosts_len - 1]
                .iter()
                .map(|host| TaskHost::Remote {
                    user: conn_user.clone(),
                    port: ssh_port as usize,
                    hosts: host.to_string(),
                })
                .collect_vec();
            info!(
                "InstallDBTaskGroup MonographDB multiple installation hosts are configured {:?}",
                dest_hosts
            );
            let upload_task = UploadTask::build_upload_data_dir_tasks(&config, dest_hosts);

            barrier.push(upload_task.len());
            executable.extend(upload_task.into_iter());

            execution_context_tuple.barrier = Some(barrier.clone());
            execution_context_tuple.executable = executable.clone();
        }

        // rm -rf cc_ng/ tx_log/
        let remote_install_dir = config.install_dir();
        let rm_log_data_cmd = format!(
            "rm -rf {remote_install_dir}/datafarm/cc_ng {remote_install_dir}/datafarm/tx_log",
        );

        let rm_log_data_task_instance = ExecCustomCommand::from_config(rm_log_data_cmd, &config);
        barrier.push(rm_log_data_task_instance.len());
        executable.extend(rm_log_data_task_instance.into_iter());
        execution_context_tuple.barrier = Some(barrier);
        execution_context_tuple.executable = executable;

        Ok(execution_context_tuple)
    }
}

#[async_trait::async_trait]
impl TaskGroup for CtrlDBTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let cmd_ref = cmd_arg.as_ref();
        let storage_provider = config.get_monograph_storage()?;

        let start_cass_if_need = (cmd_ref == "start" || cmd_ref == "restart")
            && storage_provider == StorageProvider::Cassandra;

        let mut mut_executable = if start_cass_if_need {
            CassandraCtlTask::from_config(cmd_arg.clone(), &config)
        } else {
            IndexMap::default()
        };

        let mut barrier = if !mut_executable.is_empty() {
            vec![mut_executable.len()]
        } else {
            vec![]
        };

        let batch_cmd = match cmd_arg {
            CommandArgs::Restart {
                cluster: ref cluster_name,
            } => {
                vec![
                    CommandArgs::Stop {
                        cluster: cluster_name.clone(),
                        force: Some("false".to_string()),
                    },
                    CommandArgs::Start {
                        cluster: cluster_name.to_string(),
                    },
                ]
            }
            _ => {
                vec![cmd_arg.clone()]
            }
        };

        for cmd in batch_cmd {
            let crl_task_instance = MonographCtlTask::from_config(cmd.clone(), &config);
            barrier.push(crl_task_instance.len());
            mut_executable.extend(crl_task_instance.into_iter());
        }

        let final_barrier = if start_cass_if_need {
            Some(barrier)
        } else {
            None
        };
        Ok(TaskExecutionContext {
            task_group: format!("control-{cmd_ref}"),
            barrier: final_barrier,
            executable: mut_executable,
        })
    }
}

#[async_trait::async_trait]
impl TaskGroup for CustomCmdTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let cmd_ref = cmd_arg.as_ref().to_string();
        let user_command = match cmd_arg {
            CommandArgs::Exec {
                command,
                cluster: _,
            } => command,
            _ => {
                unreachable!()
            }
        };
        let exec_cmd_task_execution = ExecCustomCommand::from_config(user_command, &config);

        Ok(TaskExecutionContext {
            task_group: cmd_ref,
            barrier: None,
            executable: exec_cmd_task_execution,
        })
    }
}

#[async_trait::async_trait]
impl TaskGroup for InstallRuntimeDepsTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<TaskExecutionContext> {
        let install_runtime_deps = RuntimeDepsInstallation::from_config(&config)?;
        Ok(TaskExecutionContext {
            task_group: cmd_arg.as_ref().to_string(),
            barrier: None,
            executable: install_runtime_deps,
        })
    }
}
