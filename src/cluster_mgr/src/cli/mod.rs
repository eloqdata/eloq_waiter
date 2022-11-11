use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use std::path::PathBuf;
use tracing::error;

pub mod cmd_base;
pub mod config;
pub mod task;

pub const MONOGRAPH_CONF: &str = "my.cnf";
pub const MONOGRAPH_CONF_TEMPLATE: &str = "my_template.cnf";
pub const START_MONOGRAPH_SCRIPT: &str = "start_monographdb.bash";
pub const START_MONOGRAPH_TEMPLATE: &str = "start_monographdb.template";
pub const MONOGRAPH_INSTALL_TEMPLATE: &str = "monograph_install_db.template";
pub const MONOGRAPH_INSTALL_SCRIPT: &str = "monograph_install_db.bash";

#[derive(Parser, Default, Debug)]
#[command(author, version = "1.0.0", about = "MonographDB Cluster Manager Cli")]
#[command(next_line_help = true)]
pub struct ClusterMgrCommandArgs {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<CommandArgs>,
}

#[derive(Subcommand, Clone, Debug, Hash, PartialEq, Eq)]
pub enum CommandArgs {
    /// Deploy the MonographDB cluster by specifying the cluster_topology.yaml file.
    Deploy {
        #[arg(short, long, value_name = "CLUSTER TOPOLOGY FILE")]
        topology_file: String,
    },
    Install {
        #[arg(short = 'c', long, value_name = "CLUSTER NAME")]
        cluster: String,
    },
    /// Start ClusterMgrCli's webservice on the specified port.
    Web {
        #[arg(short, long, value_name = "WEB SERVICE PORT")]
        port: i16,
    },
    /// Start the MonographDB cluster with the specified cluster name.
    Start {
        #[arg(short = 'l', long, value_name = "CLUSTER NAME")]
        cluster: String,
    },
    /// Stop the MonographDB cluster with the specified cluster name.
    Stop {
        #[arg(short = 'k', long, value_name = "CLUSTER NAME")]
        cluster: String,
    },
    /// Restart the MonographDB cluster with the specified cluster name.
    Restart {
        #[arg(short, long, value_name = "CLUSTER NAME")]
        cluster: String,
    },
    /// Execute custom shell commands.
    Exec {
        #[arg(short, long, value_name = "SHELL COMMAND/SCRIPT")]
        script: String,
    },
    /// Display cluster status.
    Display {
        #[arg(short = 'i', long, value_name = "CLUSTER NAME")]
        cluster: String,
    },
}

pub fn download_dir() -> PathBuf {
    let download_dir = dirs::download_dir();
    if download_dir.is_none() {
        let download_path_buf = dirs::home_dir()
            .unwrap()
            .join("Downloads")
            .join("mono-cluster-cli");
        let download_path_create_rs = std::fs::create_dir_all(download_path_buf.as_path());
        if let Err(create_err) = download_path_create_rs {
            error!("Create download path  {:?} error", download_path_buf);
            panic!("Create download path Error cause by {:?} ", create_err);
        }
        download_path_buf
    } else {
        dirs::download_dir().unwrap()
    }
}

pub fn download_file_path(download_files: Vec<String>) -> Vec<PathBuf> {
    let download_dir = download_dir();
    download_files
        .iter()
        .map(|file| download_dir.join(file.as_str()))
        .collect_vec()
}

pub fn file_process_progress(
    total_size: u64,
    file_name: String,
    process_chars: &str,
) -> ProgressBar {
    let cmd_pb = ProgressBar::new(total_size);
    let sty = format!(
        "{{spinner:.green}} {:14}: [{{elapsed_precise}}] \
        [{{wide_bar:.green/white}}] \
        {{bytes}}/{{total_bytes}} ({{eta}})",
        file_name
    );
    cmd_pb.set_style(
        ProgressStyle::default_spinner()
            .template(sty.as_str())
            .unwrap()
            .progress_chars(process_chars),
    );
    cmd_pb
}
