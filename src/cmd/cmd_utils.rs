use crate::cmd::base::{CmdStatus, Platform, SUPPORT_CMD_LIST};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::BufRead;
use std::path::Path;

//TODO: may be use fn
#[macro_export]
macro_rules! stdio_handle {
   ($stdio_closure:expr $(,$stdio_pipe:expr)*) => {{
        $(
          if let Some(std_out) = $stdio_pipe {
              let buffer_reader = std::io::BufReader::new(std_out);
              for line in buffer_reader.lines() {
                  let line = line.unwrap();
                  let stripped_line = line.trim();
                  if !stripped_line.is_empty() {
                    $stdio_closure(stripped_line);
                  }
              }
          }
        )*
   }};
}

pub fn curr_platform() -> Platform {
    Platform {
        os_type: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        family: env::consts::FAMILY.to_string(),
    }
}

pub fn default_log_handler() -> anyhow::Result<File> {
    let log = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(create_log_path_and_get());
    if log.is_ok() {
        Ok(log.ok().unwrap())
    } else {
        Err(anyhow::Error::from(log.err().unwrap()))
    }
}

pub fn get_process_bar(progress_bar_type: &str, cmd: &str) -> ProgressBar {
    match progress_bar_type {
        "pipe" => pipe_progress_bar(cmd.to_string()),
        "elapsed" => elapsed_progress_bar(None, None),
        _ => unreachable!(),
    }
}

pub fn pipe_progress_bar(cmd_str: String) -> ProgressBar {
    let cmd_pb = ProgressBar::new_spinner();
    cmd_pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!("{{spinner:.dim.bold}} {}: {{wide_msg}}", cmd_str))
            .unwrap()
            .progress_chars("##-"),
    );
    cmd_pb
}

pub fn elapsed_progress_bar(len: Option<u64>, customer_msg: Option<String>) -> ProgressBar {
    let total_size = if let Some(size) = len { size } else { 0_u64 };
    let cmd_pb = ProgressBar::new(total_size);
    let sty = if let Some(msg) = customer_msg {
        format!(
            "{{spinner:.green}} {:15}: [{{elapsed_precise}}] [{{wide_bar:.green/white}}] {{bytes}}/{{total_bytes}} ({{eta}})", msg)
    } else {
        "{spinner:.green} [{elapsed_precise}] [{wide_bar:.green/white}] {bytes}/{total_bytes} ({eta})"
            .to_string()
    };
    cmd_pb.set_style(
        ProgressStyle::default_spinner()
            .template(sty.as_str())
            .unwrap()
            .progress_chars("#>-"),
    );
    cmd_pb
}

pub fn cmd_process<F>(cmd_name: String, args: Option<Vec<String>>, mut stdout_f: F) -> CmdStatus
where
    F: FnMut(&str),
{
    let mut cmd = std::process::Command::new(cmd_name.as_str());
    if let Some(cmd_args) = args {
        for arg in cmd_args {
            cmd.arg(arg.as_str());
        }
    }
    let mut child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // .expect(format!("{} command failed to start", cmd_name).as_str());
    stdio_handle!(stdout_f, child.stderr.take());
    let exists_rs = child.wait();
    println!("ExistsRs={:?}", exists_rs);
    if let Ok(exitstatus) = exists_rs {
        CmdStatus {
            success: exitstatus.success(),
            output: None,
            status_file: None,
        }
    } else {
        CmdStatus {
            success: false,
            output: None, //Some(stderr_output),
            status_file: None,
        }
    }
}

pub fn all_support_cmd_string() -> String {
    SUPPORT_CMD_LIST
        .iter()
        .map(|cmd_str| format!("\t{}", cmd_str))
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn create_log_path_and_get() -> String {
    let curr_path = if let Ok(log_path) = env::var("MONO_WAITER_LOG") {
        log_path
    } else {
        "./.monograph_waiter/logs".to_string()
    };
    let path_buf = Path::new(&curr_path);
    let rs = std::fs::create_dir_all(path_buf.as_os_str().to_str().unwrap());
    if let Err(err) = rs {
        println!("Create Log root error path={} err={:?}", curr_path, err);
    }
    curr_path + "/monograph_waiter.log"
}
