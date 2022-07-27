use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::cmd::base::{Cmd, CmdContext, CmdStatus};
use crate::cmd::cmd_utils::{curr_platform, invoke_sys_cmd};

// Build and runtime dependencies. For now, it only supports Linux and macOS
lazy_static! {
    pub static ref DEPS: HashMap<&'static str, Vec<&'static str>> =  {
        let mut dep_mapping = HashMap::new();
        dep_mapping.insert("macos", vec!["git", "cmake", "ninja", "libuv", "glog","openssl",
            "gnu-getopt", "coreutils", "gflags", "leveldb", "gperftools", "bison"]);
        dep_mapping.insert("linux", vec!["git", "g++", "make", "libssl-dev","libgflags-dev",
            "libgoogle-glog-dev", "libprotobuf-dev", "libprotoc-dev","protobuf-compiler",
            "libleveldb-dev", "libsnappy-dev"]);
        dep_mapping
    };
}
/// Check if a monograph instance is started
/// and if the installation compilation environment matches the requirements.
#[derive(Clone, Debug)]
struct CheckEnv;

impl Cmd for CheckEnv {
    fn id() -> String {
        "check".to_string()
    }

    fn set_up(&self) -> CmdStatus {
        match curr_platform().os_type.as_str() {
            "macos" => {
                invoke_sys_cmd("command".to_string(), Some(vec!["-v".to_string(), "brew".to_string()]))
            }
            _ => {
                CmdStatus::default()
            }
        }
    }

    fn run(&mut self, context: &mut CmdContext<impl std::io::Write, impl Clone>) -> CmdStatus {
        todo!()
    }
}
