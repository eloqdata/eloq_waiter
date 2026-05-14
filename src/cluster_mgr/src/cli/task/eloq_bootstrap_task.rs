use crate::cli::ssh::SSHCommandOption::CollectOutput;
use crate::cli::ssh::SSHSession;
use crate::cli::task::task_base::{
    CmdErr, ExecutionValue, TaskArgValue, TaskExecutor, TaskHost, TaskId, TaskInstance,
};
use crate::config::config_base::{export_asan, DeployConfig};
use crate::config::deployment::Version;
use crate::task_return_value;
use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone)]
pub struct EloqInstall {
    config: DeployConfig,
    task_id: TaskId,
}

impl EloqInstall {
    pub fn from_config(
        config: &DeployConfig,
        task_host: TaskHost,
        port: String,
    ) -> IndexMap<TaskId, TaskInstance> {
        let (_, _, host) = task_host.ssh_conn_tuple();
        let task_id = TaskId {
            cmd: "install".to_string(),
            task: format!("eloq-install-{port}"),
            host,
        };
        IndexMap::from([(
            task_id.clone(),
            TaskInstance {
                task_input: HashMap::default(),
                task: Box::new(EloqInstall::new(config.clone(), task_id)),
                task_host,
            },
        )])
    }

    pub fn new(config: DeployConfig, task_id: TaskId) -> Self {
        Self { config, task_id }
    }
}

#[async_trait]
impl TaskExecutor for EloqInstall {
    fn identifier(&self) -> TaskId {
        self.task_id.clone()
    }

    async fn execute(
        &self,
        task_host: TaskHost,
        _task_arg: HashMap<String, TaskArgValue>,
    ) -> anyhow::Result<Option<ExecutionValue>> {
        info!("execute {}", self.task_id.format_string());

        let ssh_session =
            SSHSession::from_task_host(task_host, self.config.connection.ssh_auth_key().unwrap())
                .await?;
        let txsv_dir = self.config.deployment.tx_srv_home();

        let task = self.task_id.task.clone();
        let parts: Vec<&str> = task.rsplitn(2, '-').collect();
        let port = parts[0].to_string();

        let ini_file = self.config.deployment.tx_srv_ini(&port);
        let bootstarp_sh = if !ini_file.is_empty() {
            let fast_unwind_on_malloc = self.config.deployment.uses_eloqstore_storage();
            let detect_stack_use_after_return = !self.config.deployment.uses_eloqstore_storage();
            let head = if let Some(Version::Debug) = self.config.deployment.version() {
                export_asan(
                    "logs/bootstrap-asan",
                    fast_unwind_on_malloc,
                    detect_stack_use_after_return,
                )
            } else {
                format!("export LD_PRELOAD={txsv_dir}/lib/libmimalloc.so.2")
            };
            let env_exports = self.config.deployment.gen_env_exports();
            format!(
                r#"cd {txsv_dir}; mkdir -p logs/std-output; {env_exports}export \
LD_LIBRARY_PATH={txsv_dir}/lib:$LD_LIBRARY_PATH; {head}; bin/eloqkv --config={ini_file} \
--bootstrap > logs/bootstrap-{port}.log 2>&1 "#
            )
        } else {
            anyhow::bail!("cannot bootstrap EloqKV: missing config file for port {port}");
        };
        let install_rs = ssh_session.command(&bootstarp_sh, CollectOutput).await?;
        ssh_session.close().await?;
        task_return_value!(
            install_rs,
            |status_code: i32| -> CmdErr {
                CmdErr::EloqInstallErr(bootstarp_sh, status_code.to_string())
            },
            "EloqInstall",
            HashMap::from([(
                "ELOQ_DATA_DIR".to_string(),
                TaskArgValue::Str(format!("{}/datafarm", txsv_dir))
            )])
        );
    }
}
