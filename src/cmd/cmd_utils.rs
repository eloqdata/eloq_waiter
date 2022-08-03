use crate::cmd::base::{CmdStatus, Platform, SUPPORT_CMD_LIST};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::BufRead;
use std::path::Path;

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

pub fn cmd_process<F>(cmd_name: &str, args: Option<Vec<String>>, mut stdout_f: F) -> CmdStatus
where
    F: FnMut(&str),
{
    let mut cmd = std::process::Command::new(cmd_name);
    if let Some(cmd_args) = args {
        let real_args = cmd_args.iter().map(|c| c.as_str()).collect::<Vec<_>>();
        cmd.args(real_args);
    }
    let pipe_rs = os_pipe::pipe();
    if let Ok((reader, writer)) = pipe_rs {
        let writer_clone = writer.try_clone().unwrap();
        let mut child = cmd.stdout(writer).stderr(writer_clone).spawn().unwrap();
        drop(cmd);

        let buffer_reader = std::io::BufReader::new(reader);
        for line_rs in buffer_reader.lines() {
            let line = line_rs.unwrap();
            let stripped_line = line.trim();
            if !stripped_line.is_empty() {
                stdout_f(stripped_line);
            }
        }
        let child_exist_status = child.wait();
        println!("cmd exist status={:?}", child_exist_status);
        if let Ok(exitstatus) = child_exist_status {
            CmdStatus {
                success: exitstatus.success(),
                output: None,
            }
        } else {
            CmdStatus {
                success: false,
                output: None, //Some(stderr_output),
            }
        }
    } else {
        CmdStatus {
            success: false,
            output: Some(format!(
                "os_pipe::pipe() error. cause by {}",
                pipe_rs.err().unwrap()
            )),
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
