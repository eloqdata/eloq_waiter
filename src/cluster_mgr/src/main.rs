use clap::Parser;
use cluster_mgr::cli::cmd_base::CmdExecutor;
use cluster_mgr::cli::Command;
use owo_colors::OwoColorize;
use std::process::exit;
use tracing::{error, info};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let cmd = Command::parse();
    let home = CmdExecutor::home_init(cmd.home).expect("home dir init failed");
    let log_path = home.join("last.log");
    let log_file = std::fs::File::create(&log_path).expect("can't init log");
    tracing_subscriber::fmt().with_writer(log_file).init();
    let executor = Box::leak(Box::new(CmdExecutor::new(home)));
    if let Some(sub) = cmd.subcmd {
        info!("command: {:#?}", sub);
        if let Err(e) = executor.run(sub, None, cmd.quiet).await {
            error!("{}", e);
            eprintln!("{}: {e}\n logfile: {}", "failed".red(), log_path.display());
            exit(1);
        }
    }
}
