use crate::cli::task::group::{Config, MonitorCtlTaskGroup, TaskGroup};
use crate::cli::task::monitor_ctl_task::MonitorCtlTask;
use crate::cli::task::task_base::TaskExecutionContext;
use crate::cli::task::upload::upload_task_builder::{upload_tasks, UploadTaskBuilderType};
use crate::cli::SubCommand;
use indexmap::IndexMap;

#[async_trait::async_trait]
impl TaskGroup for MonitorCtlTaskGroup {
    async fn tasks(
        &self,
        cmd_arg: SubCommand,
        config: &Config,
    ) -> anyhow::Result<TaskExecutionContext> {
        let cluster_config = match config {
            Config::Cluster(cfg) => cfg,
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected ClusterConfig for MonitorCtlTaskGroup"
                ))
            }
        };

        if cluster_config.deployment.monitor.is_none() {
            return Ok(TaskExecutionContext {
                task_group: format!("control-{}", cmd_arg.as_ref()),
                barrier: None,
                executable: IndexMap::new(),
            });
        }
        let monitor_ctl_cmd = match &cmd_arg {
            SubCommand::Monitor {
                cluster: _,
                command,
            } => command,
            _ => unreachable!(),
        };
        let mut executable = IndexMap::new();
        let mut barrier = vec![];
        let exporter_task_instance =
            MonitorCtlTask::exporter_ctl_task(cmd_arg.clone(), cluster_config);
        let prometheus_task_instance =
            MonitorCtlTask::prometheus_ctl_task(cmd_arg.clone(), cluster_config);
        let grafana_task_instance =
            MonitorCtlTask::grafana_ctl_task(cmd_arg.clone(), cluster_config);

        if monitor_ctl_cmd.to_lowercase().eq("start") {
            // Re-upload the Prometheus config (and other monitor config files) before
            // starting the services so that any config changes (remote_write_urls,
            // retention_time, etc.) are applied to the running Prometheus instance.
            let monitor_conf_upload = upload_tasks(UploadTaskBuilderType::MonitorConf, config);
            if !monitor_conf_upload.is_empty() {
                barrier.push(monitor_conf_upload.len());
                executable.extend(monitor_conf_upload);
            }
        }

        barrier.push(exporter_task_instance.len());
        barrier.push(prometheus_task_instance.len());
        barrier.push(grafana_task_instance.len());

        executable.extend(exporter_task_instance);
        executable.extend(prometheus_task_instance);
        executable.extend(grafana_task_instance);

        let cmd_ref = cmd_arg.as_ref();
        Ok(TaskExecutionContext {
            task_group: format!("control-{cmd_ref}"),
            barrier: Some(barrier),
            executable,
        })
    }
}
