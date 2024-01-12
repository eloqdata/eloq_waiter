use clap::Parser;
use cluster_mgr::cli::cmd_base::CommandExecutor;
use cluster_mgr::cli::ClusterMgrCommandArgs;
use cluster_mgr::config::{CONFIG_PATH_DIR, HOME_DIR};
use std::{env, path::PathBuf, process::exit};
use tracing::{error, Level};
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let level = if let Ok(tracing_env) = env::var("MONO_CLUSTER_MGR_TRACING") {
        if !tracing_env.is_empty() && tracing_env.to_lowercase() == "true" {
            Level::INFO
        } else {
            Level::WARN
        }
    } else {
        Level::WARN
    };
    println!("ClusterMgr Tracing Level = {level:?}");
    let filter = EnvFilter::from_default_env()
        .add_directive("russh::client::encrypted=warn".parse().unwrap());
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_env_filter(filter)
        .init();
    let cluster_mgr_cmd = ClusterMgrCommandArgs::parse();
    match cluster_mgr_cmd.home {
        Some(ref home) => env::set_var(HOME_DIR, home),
        None => {
            if env::var(HOME_DIR).is_err() {
                env::set_var(HOME_DIR, env::current_dir().unwrap())
            }
        }
    };
    // Set config directory path
    let cnf_dir = PathBuf::from(env::var(HOME_DIR).unwrap()).join("config");
    if !cnf_dir.exists() {
        error!("Config path not exist: {} ", cnf_dir.display());
        exit(1);
    }
    env::set_var(CONFIG_PATH_DIR, cnf_dir);

    let cmd_executor = Box::leak(Box::new(CommandExecutor::new()));
    if let Some(command) = cluster_mgr_cmd.command {
        println!("ClusterMgr receive {:?} command", command.clone());
        if let Err(e) = cmd_executor.run(command, None).await {
            error!("{}", e);
            exit(1);
        }
    }
}
