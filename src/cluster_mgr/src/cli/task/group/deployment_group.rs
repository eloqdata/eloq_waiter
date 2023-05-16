use indexmap::IndexMap;
use itertools::Itertools;
use crate::cli::CommandArgs;
use crate::cli::task::download_task::DownloadFromRemoteTask;
use crate::cli::task::group::{DeploymentTaskGroup, TaskGroup};
use crate::cli::task::local_copy_task::LocalCopyTask;
use crate::cli::task::task_base::{TaskExecutionContext, TaskId, TaskInstance};
use crate::cli::task::unpack_file_task::UnpackFileTask;
use crate::cli::task::upload::upload_task_builder::{upload_tasks, UploadTaskBuilderType};
use crate::config::config_base::{DEPLOYMENT_CHECK_SUCCESS_TASK, DeploymentConfig};
use crate::state::state_mgr::STATE_MGR;

impl DeploymentTaskGroup {
    fn skip_success_task_execution(
        task_instances: &IndexMap<TaskId, TaskInstance>,
        success_task_ids: &[TaskId],
    ) -> IndexMap<TaskId, TaskInstance> {
        if success_task_ids.is_empty() {
            task_instances.clone()
        } else {
            task_instances
                .iter()
                .filter(|(task_id, _)| !success_task_ids.contains(task_id))
                .map(|(task_id, task_instance)| (task_id.clone(), task_instance.clone()))
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

        let success_task_vec = success_task_entity
            .iter()
            .map(|task_status_entity| {
                let task_id_string = &task_status_entity.task;
                TaskId::from_json_string(task_id_string.clone())
            })
            .collect_vec();

        let download_task = DownloadFromRemoteTask::from_config(&config)?;
        let mut copy_or_download_task_instances = LocalCopyTask::form_config(&config)?;
        copy_or_download_task_instances.extend(download_task.into_iter());

        let need_skip_success_task = if let Some(ref opts) = config.conf_opts {
            if let Some(check) = opts.get(DEPLOYMENT_CHECK_SUCCESS_TASK) {
                *check
            } else {
                true
            }
        } else {
            true
        };
        let (upload_task, unpack_task) = if need_skip_success_task {
            (
                DeploymentTaskGroup::skip_success_task_execution(
                    &upload_tasks(UploadTaskBuilderType::InstallTar, &config),
                    &success_task_vec,
                ),
                DeploymentTaskGroup::skip_success_task_execution(
                    &UnpackFileTask::from_config(&config)?,
                    &success_task_vec,
                ),
            )
        } else {
            (
                upload_tasks(UploadTaskBuilderType::InstallTar, &config),
                UnpackFileTask::from_config(&config)?,
            )
        };

        let mut upload_monitor_tasks = upload_tasks(UploadTaskBuilderType::MonitorConf, &config);
        let upload_mysql_exporter_tasks =
            upload_tasks(UploadTaskBuilderType::MySQLExporter, &config);
        upload_monitor_tasks.extend(upload_mysql_exporter_tasks.into_iter());
        let barrier = Some(vec![
            copy_or_download_task_instances.len(),
            upload_task.len(),
            unpack_task.len(),
            upload_monitor_tasks.len(),
        ]);
        let mut executable = IndexMap::new();
        executable.extend(copy_or_download_task_instances.into_iter());
        executable.extend(upload_task.into_iter());
        executable.extend(unpack_task.into_iter());
        executable.extend(upload_monitor_tasks.into_iter());
        Ok(TaskExecutionContext {
            task_group: cmd_ref,
            barrier,
            executable,
        })
    }
}