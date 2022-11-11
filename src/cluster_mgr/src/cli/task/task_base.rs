use crate::cli::config::{load_remote_env, DeploymentConfig};
use crate::cli::task::task_group::TASK_GROUP;
use crate::cli::CommandArgs;
use crate::enum_into_trait;
use crate::state::state_base::StateOperation;
use crate::state::state_mgr::{STATE_MGR, TASK_STATUS_STATE};
use crate::state::task_status_operation::{TaskStatusEntity, TaskStatusOperation};
use anyhow::anyhow;
use async_trait::async_trait;
use dyn_clone::DynClone;
use futures::StreamExt;
use futures_async_stream::try_stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::string::ToString;
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tracing::{error, info};
use ExecutionResult as LastResult;

pub type EnvProperties = HashMap<String, String>;

pub(crate) static REMOTE_ENV_PROPS: LazyLock<anyhow::Result<EnvProperties>> =
    LazyLock::new(|| load_remote_env(None));

enum_into_trait! {TaskValueInto, task_value_into, TaskValue}

#[macro_export]
macro_rules! task_execute_post {
    ($execution_rs:expr, $cluster:expr, $task_mame:expr, $command:expr, $task_host:expr) => {{
        let status_tuple = if let Ok(execution) = $execution_rs {
            (0, execution)
        } else {
            (1, None)
        };

        let task_status_entity = TaskStatusEntity {
            cluster_name: $cluster,
            task: String::from($task_mame),
            command: String::from($command),
            task_host: String::from($task_host),
            task_status: status_tuple.0,
            create_timestamp: Default::default(),
            update_timestamp: Default::default(),
        };
        save_task_status(task_status_entity, status_tuple.1).await
    }};
}

#[macro_export]
macro_rules! task_value_impl {
    ($({$type_var:ident, $task_type:ty}),*) => {
       $(impl TaskValueInto for $task_type {
           fn task_value_into(task_type: TaskValue) -> Self {
              match task_type {
                 TaskValue::$type_var(value) => value,
                 _ => unreachable!(),
              }
           }
        })*
    };
}

task_value_impl! {
    {Str, String},
    {Number, usize},
    {List, Vec<String>}
}

#[derive(PartialEq, Eq, Clone, Error, Debug)]
pub enum CmdErr {
    #[error("Found cli execution failed, error cause {0}")]
    TasksErr(String),
    #[error("Task [{0}] execution failed, error cause {1}")]
    RunErr(String, String),
    #[error("Download file failed, download URL {0} , error causes {1}")]
    DownloadErr(String, String),
    #[error("Upload file failed, local file {0}, error causes {1}")]
    UploadErr(String, String),
    #[error("Error establishing ssh connection, user@host={0}, error causes {1}")]
    SSHConnErr(String, String),
    #[error("SSHConn execute remote cmd {0} failed, error causes {1}")]
    SSHRemoteCmdErr(String, String),
    #[error("Error executing apache-cassandra control command {0} failed, error causes {1}")]
    CassandraCtlErr(String, String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskValue {
    Str(String),
    Number(usize),
    List(Vec<String>),
}

impl TaskValue {
    pub fn into_inner_value<T: TaskValueInto>(self) -> T {
        TaskValueInto::task_value_into(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskHost {
    Local,
    Remote {
        user: String,
        port: usize,
        hosts: String,
    },
}

impl TaskHost {
    pub fn ssh_conn_tuple(&self) -> (String, usize, String) {
        match self {
            TaskHost::Local => ("_local".to_string(), 22, "localhost".to_string()),
            TaskHost::Remote { user, port, hosts } => (user.clone(), *port, hosts.clone()),
        }
    }
}

pub type ExecutionResult = HashMap<String, TaskValue>;

static FINISH_: LazyLock<LastResult> = LazyLock::new(|| {
    HashMap::from([("_FINISH_SIGNAL".to_string(), TaskValue::Str("".to_string()))])
});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskId {
    pub cmd: String,
    pub task: String,
}

impl TaskId {
    pub fn as_string(&self) -> String {
        let task_id_string = serde_json::to_string(self);
        task_id_string.unwrap()
    }
}

#[async_trait]
pub trait TaskExecutor: 'static + Send + Sync + DynClone {
    fn identifier(&self) -> TaskId;

    async fn execute(
        &self,
        task_host: TaskHost,
        task_arg: HashMap<String, TaskValue>,
    ) -> anyhow::Result<Option<ExecutionResult>>;
}

dyn_clone::clone_trait_object!(TaskExecutor);

#[derive(Clone)]
pub struct TaskExecutionContext {
    pub(crate) task_input: HashMap<String, TaskValue>,
    pub(crate) task: Box<dyn TaskExecutor>,
    pub(crate) task_host: TaskHost,
}

#[derive(Clone)]
pub struct TaskExecutionContextTuple {
    pub barrier: Option<Vec<usize>>,
    pub executable: Vec<TaskExecutionContext>,
}

#[derive(Clone)]
struct TaskController {
    rx: crossbeam_channel::Receiver<anyhow::Result<Option<ExecutionResult>>>,
    tx: crossbeam_channel::Sender<anyhow::Result<Option<ExecutionResult>>>,
}

impl TaskController {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::bounded(2000);
        Self { rx, tx }
    }

    fn task_split(
        barrier: Option<Vec<usize>>,
        tasks: Vec<TaskExecutionContext>,
    ) -> Vec<&'static [TaskExecutionContext]> {
        let tasks = Box::leak(Box::new(tasks));
        if barrier.is_none() {
            vec![tasks.as_slice()]
        } else {
            let barrier_array = barrier.as_ref().unwrap();
            let mut begin;
            let mut end = 0;
            let mut all_split = vec![];
            for (idx, barrier_val) in barrier_array.iter().enumerate() {
                if idx == 0 {
                    begin = 0;
                    end = *barrier_val;
                } else {
                    begin = end;
                    end = begin + *barrier_val;
                }
                info!("TaskController run_task_split {}..{}", begin, end);
                let task_slice = &tasks[begin..end];
                all_split.push(task_slice);
            }
            all_split
        }
    }

    #[try_stream(boxed, ok = Option<ExecutionResult>, error = anyhow::Error)]
    pub async fn try_stream(self) {
        while let Ok(rs) = self.rx.recv() {
            if rs.is_err() {
                error!("TaskController try_stream receive error {:?}", rs.err());
                break;
            } else {
                let execute_rs = rs.unwrap();
                if let Some(execution_rs) = execute_rs.as_ref() {
                    if execution_rs.contains_key("_FINISH_SIGNAL") {
                        break;
                    }
                }
                yield execute_rs;
            }
        }
    }

    async fn run_task_split(
        &'static self,
        splits: &'static [TaskExecutionContext],
        config: DeploymentConfig,
    ) {
        let mut joins = vec![];
        splits
            .iter()
            .enumerate()
            .for_each(|(_idx, execution_context)| {
                let tx_arc = Arc::new(&self.tx);
                let config_arc = Box::leak(Box::new(config.clone().deployment));
                let join = tokio::task::spawn(async move {
                    info!("CurrentThread = {:?}", std::thread::current().id());
                    let input = &execution_context.task_input;
                    let task = &execution_context.task;
                    let task_host = &execution_context.task_host;
                    let execution_rs = task.execute(task_host.clone(), input.clone()).await;

                    let task_id = task.identifier();
                    let cmd = task_id.clone().cmd;
                    let conn_tuple = task_host.ssh_conn_tuple();
                    let cluster_name = config_arc.cluster_name.clone();
                    // execution_rs,cluster,task_mame,command,task_host
                    let post_execute_rs = task_execute_post!(
                        execution_rs,
                        cluster_name.clone(),
                        task_id.as_string(),
                        cmd.as_str(),
                        conn_tuple.2
                    );
                    //let post_execute_rs = execution_context.task.post_execute(execution_rs).await;
                    let tx_cloned = tx_arc.clone();
                    info!(
                        "TaskController Send result = {:?} to channel",
                        post_execute_rs
                    );
                    tx_cloned.send(post_execute_rs).unwrap()
                });
                joins.push(join);
            });
        let join_all = futures::future::join_all(joins).await;
        info!("TaskController task split run complete. {:?}", join_all);
    }

    pub async fn run_all_tasks(
        &'static self,
        barrier: Option<Vec<usize>>,
        tasks: Vec<TaskExecutionContext>,
        config: DeploymentConfig,
    ) {
        let split = TaskController::task_split(barrier, tasks);
        for task_split in split.into_iter() {
            self.run_task_split(task_split, config.clone()).await;
        }
        self.tx.send(Ok(Some(FINISH_.clone()))).unwrap()
    }
}

#[derive(Clone)]
pub struct TaskMgr {
    task_controller: TaskController,
}

impl Default for TaskMgr {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskMgr {
    pub fn new() -> Self {
        Self {
            task_controller: TaskController::new(),
        }
    }
}

impl TaskMgr {
    pub async fn receive_task_result(&'static self) {
        let mut result_reader = self.task_controller.clone().try_stream();
        while let Some(Ok(rs)) = result_reader.next().await {
            info!("TaskMgr receive task execute result = {:?}", rs);
        }
    }

    async fn get_task_group_and_run(&'static self, group_key: &str, config: DeploymentConfig) {
        let task_group = TASK_GROUP.get(group_key).unwrap();
        let tasks_execution = task_group.tasks(config.clone()).unwrap();
        self.task_controller
            .run_all_tasks(tasks_execution.barrier, tasks_execution.executable, config)
            .await;
    }

    pub async fn build_and_run(
        &'static self,
        cmd: CommandArgs,
        config: DeploymentConfig,
    ) -> anyhow::Result<()> {
        match cmd.clone() {
            CommandArgs::Deploy { topology_file: _ } => {
                self.get_task_group_and_run("Deploy", config).await;
            }
            CommandArgs::Install { cluster: _ } => {
                self.get_task_group_and_run("Install", config).await;
            }
            _ => {
                unimplemented!()
            }
        }
        Ok(())
    }
}

pub(crate) async fn save_task_status(
    task_status_entity: TaskStatusEntity,
    execution_result: Option<ExecutionResult>,
) -> anyhow::Result<Option<ExecutionResult>> {
    let state_operation = STATE_MGR.get_state_operation::<TaskStatusOperation>(TASK_STATUS_STATE);

    let put_rs = state_operation.put(task_status_entity).await;
    if let Err(put_err) = put_rs {
        let err_string = put_err.to_string();
        Err(anyhow!(err_string))
    } else {
        Ok(execution_result)
    }
}
