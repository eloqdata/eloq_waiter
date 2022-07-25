pub mod check_env;

use anyhow::Result;
use thiserror::Error;

const MAC_DEPENDENCIES: &'static [&'static str] = &["git", "cmake", "ninja", "libuv", "glog",
    "openssl", "gnu-getopt", "coreutils", "gflags", "leveldb", "gperftools", "bison"];

const LINUX_DEPENDENCIES: &'static [&'static str] = &["git", "g++", "make", "libssl-dev",
    "libgflags-dev", "libgoogle-glog-dev", "libprotobuf-dev", "libprotoc-dev",
    "protobuf-compiler", "libleveldb-dev", "libsnappy-dev"];

pub trait Command {
    fn set_up(&self) {
        println!("Cmd is not required.");
    }

    fn execute(&self) -> Option<Result<String>>;
}

#[derive(Error, Debug)]
pub enum CmdErrorCode {
    #[error("For now only support Linux and MacOS. current OS is {0}")]
    UnSupportOS(String)
}
