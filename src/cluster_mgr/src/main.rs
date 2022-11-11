use clap::Parser;
use cluster_mgr::cli::cmd_base::CommandExecutor;
use cluster_mgr::cli::config::CONFIG_PATH_DIR;
use cluster_mgr::cli::ClusterMgrCommandArgs;
use std::env;
use tracing::{error, info, Level};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    info!("tracing inti info.");
    let cluster_mgr_cmd = ClusterMgrCommandArgs::parse();
    let config_path = match cluster_mgr_cmd.config {
        Some(ref config) => config.to_str().unwrap().to_string(),
        None => {
            let current_dir = env::current_dir().unwrap();
            let config_path_buf = current_dir.join("config");
            let config_path = config_path_buf.as_path();
            if !config_path.exists() {
                error!(
                    "The config [{:?}] folder was not found in the current process's directory",
                    config_path_buf
                );
                return;
            } else {
                config_path.to_str().unwrap().to_string()
            }
        }
    };
    env::set_var(CONFIG_PATH_DIR, config_path.clone());
    info!("ClusterMgr found config path is {:?}", config_path);
    let cmd_executor = Box::leak(Box::new(CommandExecutor::new()));
    if let Some(command) = cluster_mgr_cmd.command {
        info!("ClusterMgr receive {:?} command", command.clone());
        let rs = cmd_executor.run(command).await;
        assert!(rs.is_ok());
    }
}
