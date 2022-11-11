use crate::cli::config::DeploymentConfig;
use crate::cli::task::cassandra_ctl_task::CassandraCtlTask;
use crate::cli::task::download_task::DownloadTask;
use crate::cli::task::task_base::TaskExecutionContextTuple;
use crate::cli::task::unpack_file_task::UnpackFileTask;
use crate::cli::task::upload_task::UploadTask;
use crate::cli::CommandArgs;
use dyn_clone::DynClone;
use std::collections::HashMap;
use std::sync::LazyLock;

pub trait TasksGroup: Send + Sync + DynClone {
    fn tasks(&self, config: DeploymentConfig) -> anyhow::Result<TaskExecutionContextTuple>;
}

dyn_clone::clone_trait_object!(TasksGroup);

#[macro_export]
macro_rules! task_group_boxed {
    ($({$task_group:ident}),*) => {
        $(
        #[derive(Clone)]
        struct $task_group;

        impl $task_group {
            fn boxed() -> Box<dyn TasksGroup> {
                Box::new(Self {})
            }
        }
        )*
    };
}

task_group_boxed! {
    {DeploymentTasksGroup},
    {InstallDBTaskGroup}
}

pub static TASK_GROUP: LazyLock<HashMap<String, Box<dyn TasksGroup>>> = LazyLock::new(|| {
    HashMap::from([
        ("Deploy".to_string(), DeploymentTasksGroup::boxed()),
        ("Install".to_string(), InstallDBTaskGroup::boxed()),
    ])
});

impl TasksGroup for DeploymentTasksGroup {
    fn tasks(&self, config: DeploymentConfig) -> anyhow::Result<TaskExecutionContextTuple> {
        let download_execution = DownloadTask::from_config(&config)?;
        let upload_execution = UploadTask::from_config(&config)?;
        let unpack_execution = UnpackFileTask::from_config(&config)?;

        let barrier = vec![
            download_execution.len(),
            upload_execution.len(),
            unpack_execution.len(),
        ];
        let executable = [download_execution, upload_execution, unpack_execution].concat();

        Ok(TaskExecutionContextTuple {
            barrier: Some(barrier),
            executable,
        })
    }
}

impl TasksGroup for InstallDBTaskGroup {
    fn tasks(&self, config: DeploymentConfig) -> anyhow::Result<TaskExecutionContextTuple> {
        let install_cmd = CommandArgs::Install {
            cluster: config.clone().deployment.cluster_name,
        };
        let cassandra_start = CassandraCtlTask::from_config(install_cmd, &config);
        //let executable = vec![cassandra_start];
        Ok(TaskExecutionContextTuple {
            barrier: None,
            executable: cassandra_start,
        })
    }
}
