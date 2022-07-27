use std::env;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::process::Command;
use indicatif::ProgressBar;
use thiserror::Error;
use crate::cmd::cmd_utils::{get_process_bar, invoke_sys_cmd};

#[derive(Error, Debug)]
pub enum CmdErrorCode {
    #[error("For now only support Linux and MacOS. current OS is {0}")]
    UnSupportOS(String)
}

#[derive(Clone, Debug)]
pub struct CmdDesc<Data> where Data: Clone + ?Sized {
    name: String,
    args: Option<Vec<String>>,
    show_progress_type: Option<String>,
    payload: Option<Data>,
}

pub trait Cmd: 'static + Send {
    /// Command unique identifier
    fn id() -> String;
    /// The action is executed before the command is executed. For example, modifying configuration files,
    /// setting environment variables, etc., is not required to implement
    fn set_up(&self) -> CmdStatus {
        CmdStatus::default()
    }
    /// Execute the command, e.g.: brew info leveldb
    fn run(&mut self, context: &mut CmdContext<impl std::io::Write, impl Clone>) -> CmdStatus;
    /// Actions executed after the command finishes running,
    /// such as cleaning up specific resources, are not required to be implemented.
    fn tear_down(&self) -> CmdStatus {
        CmdStatus::default()
    }
}

#[derive(Clone, Debug)]
pub struct Platform {
    pub os_type: String,
    pub arch: String,
    pub family: String,
}

#[derive(Clone, Debug)]
pub struct CmdStatus {
    pub(crate) success: bool,
    pub(crate) output: String,
    pub(crate) status_file: Option<PathBuf>,
}

impl Display for CmdStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prefix = if self.success {
            "✅"
        } else {
            "❗"
        };
        write!(f, "\n{} \n{}",
               format!("{} success:{}, status_file:{:?}", prefix, self.success, self.status_file),
               format!("{} {}", "📖", self.output),
        )
    }
}

impl Default for CmdStatus {
    fn default() -> Self {
        Self {
            success: true,
            output: "".to_string(),
            status_file: None,
        }
    }
}


#[derive(Clone, Debug)]
pub struct CmdContext<Log, Data> where Log: std::io::Write, Data: Clone + ?Sized {
    cmd: CmdDesc<Data>,
    log: Log,
}

impl<Log, Data> CmdContext<Log, Data> where Log: std::io::Write, Data: Clone + ?Sized {
    pub fn new(log: Log, cmd: CmdDesc<Data>) -> Self {
        Self {
            cmd,
            log,
        }
    }

    pub fn record_context<F>(&self, progress_fn: Option<F>) where F: Fn(ProgressBar, Data) {
        if let Some(progress_bar_type) = &self.cmd.show_progress_type {
            let mut cmd = Command::new(&self.cmd.name.as_str());
            if let Some(cmd_args) = &self.cmd.args {
                for arg in cmd_args {
                    cmd.arg(arg.as_str());
                }
            }
            let pb = get_process_bar(progress_bar_type.as_str(), self.cmd.name.as_str());
            let mut child = cmd.stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            progress_fn.unwrap()(pb, self.cmd.clone().payload.unwrap());
            if let Ok(sys_cmd_rs) = child.wait() {
            } else {}
        } else {
            invoke_sys_cmd(self.cmd.name.clone(), self.cmd.args.clone());
        }
    }

    fn create_log_path_and_get(cmd: &str) -> String {
        let curr_path = if let Ok(log_path) = env::var("MONO_WAITER_LOG") {
            log_path
        } else {
            "~/.monograph_waiter/log".to_string()
        };
        println!("MonoWaiter Cmd LogPath = {}", curr_path);
        let path_buf = Path::new(&curr_path).join(format!("{}.log", cmd.to_string()));
        let _rs = std::fs::create_dir_all(path_buf.as_os_str().to_str().unwrap());
        curr_path.clone()
    }
}

