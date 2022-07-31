use crate::cmd::base::{Cmd, CmdContext, CmdDesc, CmdStatus, MONO_WATER_CONF};
use crate::config::load_config;
use std::io::Write;

pub struct BuildWorkspace;

impl Cmd for BuildWorkspace {
    fn cmd_desc(&self) -> CmdDesc {
        let config_path = std::env::var(MONO_WATER_CONF).unwrap_or_else(|_| {
            panic!("Maybe it's a bug.The path to the configuration file must exist")
        });

        CmdDesc {
            name: config_path,
            args: None,
            show_progress_type: Some("elapsed".to_string()),
        }
    }

    fn exec(&self, context: &mut CmdContext<impl Write>) -> CmdStatus {
        let cmd_desc = context.get_cmd_desc();
        let base_config= cmd_desc.name;
        let _config_mapping = load_config(base_config.as_str());
        CmdStatus::default()
    }
}
