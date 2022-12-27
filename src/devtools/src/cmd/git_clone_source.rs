use crate::cmd::base::{CmdDef, CmdStatus};
use crate::cmd::cmd_utils::cmd_process;
use crate::{extract_config_value, git_clone};
use futures_util::future::join_all;
use std::sync::Arc;
use tokio::sync::Semaphore;

// if the network situation is good, this restriction is not necessary.
static GIT_CLONE_SEMAPHORE: usize = 3;

pub enum GitRepository {
    MonographDB,
    BuildAndRunDeps,
    All,
}

pub struct GitCloneSource {
    git_repository: GitRepository,
}

impl Default for GitCloneSource {
    fn default() -> Self {
        GitCloneSource::new(GitRepository::All)
    }
}

impl GitCloneSource {
    pub fn new(git_repository: GitRepository) -> Self {
        Self { git_repository }
    }

    pub async fn exec(&self) -> Vec<(CmdDef, CmdStatus<()>)> {
        let common = extract_config_value!("common", Common, "".to_string());
        let mut git_clone_status = vec![];
        let git = common.clone().compile.git;
        let git_clone_cmd_vec = match self.git_repository {
            GitRepository::All => {
                git_clone!(
                    git,
                    brpc,
                    braft,
                    catch2,
                    rocksdb,
                    tx_service,
                    log_service,
                    monograph,
                    aws,
                    cass,
                    mariadb
                )
            }
            GitRepository::BuildAndRunDeps => {
                git_clone!(git, brpc, braft, rocksdb, aws)
            }
            GitRepository::MonographDB => {
                git_clone!(git, tx_service, log_service, monograph, cass, mariadb)
            }
        };
        let mut join_all_task_vec = Vec::new();
        let semaphore = Arc::new(Semaphore::new(GIT_CLONE_SEMAPHORE));
        for git_clone_cmd in git_clone_cmd_vec.clone() {
            println!("{}", git_clone_cmd);
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let task_handler = tokio::task::spawn(async move {
                let cmd_status = cmd_process(git_clone_cmd.clone(), |stdout| {
                    println!("{}", stdout);
                });
                drop(permit);
                (git_clone_cmd, cmd_status)
            });
            join_all_task_vec.push(task_handler);
        }
        let git_clone_all_rs = join_all(join_all_task_vec).await;
        println!("{:?}", git_clone_all_rs);
        for rs in git_clone_all_rs {
            git_clone_status.push(rs.unwrap());
        }
        git_clone_status
    }
}
