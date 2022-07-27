use std::env;
use indicatif::{ProgressBar, ProgressStyle};
use crate::cmd::base::{CmdStatus, Platform};

pub fn curr_platform() -> Platform {
    Platform {
        os_type: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        family: env::consts::FAMILY.to_string(),
    }
}

pub fn get_process_bar(progress_bar_type: &str, cmd: &str) -> ProgressBar {
    match progress_bar_type {
        "pipe" => pipe_progress_bar(cmd.to_string()),
        "elapsed" => elapsed_progress_bar(),
        _ => unreachable!()
    }
}

pub fn pipe_progress_bar(cmd_str: String) -> ProgressBar {
    let cmd_pb = ProgressBar::new_spinner();
    cmd_pb.enable_steady_tick(200);
    cmd_pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("/|\\- ")
            .template(&format!("{{spinner:.dim.bold}} {}: {{wide_msg}}", cmd_str)),
    );
    cmd_pb
}

pub fn elapsed_progress_bar() -> ProgressBar {
    let cmd_pb = ProgressBar::new(0_u64);
    cmd_pb.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.green/white}] {bytes}/{total_bytes} ({eta})")
        .progress_chars("#>-"));
    cmd_pb
}

pub fn invoke_sys_cmd(cmd_name: String, args: Option<Vec<String>>) -> CmdStatus {
    let mut sys_cmd = std::process::Command::new(cmd_name.as_str());
    if let Some(ref cmd_args) = args {
        for arg in cmd_args {
            sys_cmd.arg(arg.as_str());
        }
    }
    let cmd_output = sys_cmd.output();
    if let Ok(output) = cmd_output {
        let cmd_succ = output.status.success();
        let mut all_output = String::from_utf8_lossy(&output.stdout).to_string();
        all_output.extend(String::from_utf8_lossy(&output.stderr).chars());
        let final_output = if all_output.is_empty() {
            "None Output".to_string()
        } else {
            all_output
        };
        CmdStatus {
            success: cmd_succ,
            output: final_output,
            status_file: None,
        }
    } else {
        println!("Exec Cmd {} {:?} Error", cmd_name.clone(), args);
        CmdStatus {
            success: false,
            output: format!("❗ {} {:?} ERR:{}", cmd_name, args, cmd_output.err().unwrap()),
            status_file: None,
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::cmd::cmd_utils::*;

    #[test]
    pub fn test_invoke_sys_cmd() {
        let cmd_status = invoke_sys_cmd("brew".to_string(), Some(vec!["info".to_string(), "rocksdb".to_string()]));
        println!("{}", cmd_status);
    }

    #[test]
    pub fn test_cmd_run_with_child() {}
}