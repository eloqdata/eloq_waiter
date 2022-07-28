use crate::cmd::cmd_utils::{cmd_process, get_process_bar};
use lazy_static::lazy_static;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::PathBuf;
use thiserror::Error;
use std::collections::HashMap;
use crate::cmd::check_env::CheckEnv;

lazy_static! {
    pub static ref SUPPORT_CMD_LIST: Vec<&'static str> = vec![
        "check",
        "fetch_source",
        "build",
        "playground",
        "stop_all",
        "start_all"
    ];

    pub static ref CMD_DESC_MAP : HashMap<&'static str, CmdDesc> = {
        let mut cmd_desc_mapping = HashMap::new();
        cmd_desc_mapping.insert("check", CheckEnv::cmd_desc());
        cmd_desc_mapping
    };
}

#[macro_export]
macro_rules! output_handle {
    ($cmd_output:expr, $output_by_line:expr, $has_output_post:expr) => {{
        let mut output_vec: Vec<String> = Vec::default();
        let buffer_reader = std::io::BufReader::new($cmd_output);
        for line in buffer_reader.lines() {
            let line = line.unwrap();
            let stripped_line = line.trim();
            if !stripped_line.is_empty() {
                $output_by_line(stripped_line);
            }
            if $has_output_post {
                output_vec.push(stripped_line.to_string() + "\n");
            }
        }
        output_vec
    }};
}

#[derive(Error, Debug)]
pub enum CmdErrorCode {
    #[error("For now only support Linux and MacOS. current OS is {0}")]
    UnSupportOS(String),
}

#[derive(Clone, Debug)]
pub struct CmdDesc {
    pub name: String,
    pub args: Option<Vec<String>>,
    pub show_progress_type: Option<String>,
}

pub trait Cmd: 'static + Send {
    /// Command unique identifier
    fn cmd_desc() -> CmdDesc;
    /// The action is executed before the command is executed. For example, modifying configuration files,
    /// setting environment variables, etc., is not required to implement
    fn set_up(&self) -> CmdStatus {
        CmdStatus::default()
    }
    /// Execute the command, e.g.: brew list leveldb
    fn run(&self, context: &mut CmdContext<impl Write>) -> CmdStatus {
        context.record_context()
    }
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
    pub(crate) output: Option<String>,
    pub(crate) status_file: Option<PathBuf>,
}

impl Display for CmdStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let prefix = if self.success { "✅" } else { "❗" };
        write!(
            f,
            "{}",
            format_args!(
                "{} success:{}, status_file:{:?}, {:?}",
                prefix, self.success, self.status_file, self.output
            ),
        )
    }
}

impl Default for CmdStatus {
    fn default() -> Self {
        Self {
            success: true,
            output: None,
            status_file: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CmdContext<Log>
    where
        Log: Write,
{
    cmd: CmdDesc,
    log: Log,
}

impl<Log> CmdContext<Log>
    where
        Log: Write,
{
    pub fn new(cmd_desc: CmdDesc, log: Log) -> Self {
        Self { cmd: cmd_desc, log }
    }

    pub fn record_context(&mut self) -> CmdStatus {
        let cmd_status = if let Some(progress_type) = self.cmd.clone().show_progress_type {
            let pb = get_process_bar(progress_type.as_str(), self.cmd.name.as_str());
            cmd_process(
                self.cmd.clone().name,
                self.cmd.clone().args,
                |output_by_line: &str| {
                    pb.set_message(output_by_line.to_owned());
                },
            )
        } else {
            cmd_process(
                self.cmd.clone().name,
                self.cmd.clone().args,
                |output_by_line: &str| {
                    println!("{}", output_by_line);
                },
            )
        };
        let cmd_status_log = writeln!(self.log, "Command={:?}, Status={}", self.cmd, cmd_status);
        println!("Write Log Rs={:?}", cmd_status_log);
        cmd_status
    }
}
