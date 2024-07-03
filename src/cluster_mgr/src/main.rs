use clap::Parser;
use cluster_mgr::cli::cmd_base::CommandExecutor;
use cluster_mgr::cli::ClusterMgrCommandArgs;
use std::process::exit;
use tracing::{error, info};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let args = ClusterMgrCommandArgs::parse();
    let home = CommandExecutor::home_init(args.home).expect("home dir init failed");
    let log_file = std::fs::File::create(home.join("last.log")).expect("can't init log");
    tracing_subscriber::fmt().with_writer(log_file).init();
    let executor = Box::leak(Box::new(CommandExecutor::new(home)));
    if let Some(command) = args.command {
        info!("command: {:#?}", command);
        if let Err(e) = executor.run(command, None).await {
            error!("{}", e);
            exit(1);
        }
    }
}
