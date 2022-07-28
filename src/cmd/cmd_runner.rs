use std::collections::HashMap;
use std::fs::File;
use crate::cmd::base::{CMD_DESC_MAP, CmdContext};

pub struct CmdRunner<'s> {
    cmd_processor: HashMap<String, CmdContext<&'s File>>,
}

impl<'s> CmdRunner<'s> {
    pub fn new(log: &'s File) -> Self {
        let mut cmd_processor: HashMap<String, CmdContext<&'s File>> = HashMap::new();
        for entry in CMD_DESC_MAP.iter() {
            cmd_processor.insert(entry.0.to_string(), CmdContext::new(entry.1.clone(), log));
        }
        Self {
            cmd_processor
        }
    }
}