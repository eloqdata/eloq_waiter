#![feature(once_cell)]

use clap::Parser;
use rest_api::server::CliMgrHttpServer;
use rest_api::ServerCommandArgs;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let server_cmd_args = ServerCommandArgs::parse();
    info!("MonoClusterREST command args = {:?}", server_cmd_args);
    let config_path = server_cmd_args.config.to_str().unwrap().to_string();
    let svr_static = Box::leak(Box::new(CliMgrHttpServer::new()));
    svr_static
        .start(server_cmd_args.addr, server_cmd_args.port, config_path)
        .await?;
    Ok(())
}
