// use crate::cli::cmd_base::CommandExecutor;
use serde::{Deserialize, Serialize};

use serde_json::Value;
// use std::sync::LazyLock;

pub mod server;
mod web_handler;

pub(crate) static SUPPORT_CMD: [&str; 5] = ["deploy", "install", "start", "stop", "status"];

#[derive(Deserialize, Serialize)]
pub struct Response {
    code: usize,
    msg: String,
    data: Value,
}

impl Response {
    fn succ_def() -> Self {
        Self {
            code: 200,
            msg: "".to_string(),
            data: Value::Null,
        }
    }
}

// pub(crate) static CMD_EXECUTOR: LazyLock<CommandExecutor> =
//     LazyLock::new(|| CommandExecutor::default());
