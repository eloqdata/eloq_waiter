use std::env;
use anyhow::Result;
use crate::cmd::CmdErrorCode::UnSupportOS;
use crate::cmd::Command;

#[derive(Clone, Debug)]
/// Check if a monograph instance is started
/// and if the installation compilation environment matches the requirements.
struct CheckEnvCmd;

impl CheckEnvCmd {
    pub fn check_dep(&self, os: String) -> Result<()> {
        Ok(())
    }
}

impl Command for CheckEnvCmd {
    fn execute(&self) -> Option<Result<String>> {
        let os_type = env::consts::OS;
        println!("current os type = {},arch = {}", os_type, env::consts::ARCH);
        if os_type.eq("windows") {
            Some(Err(UnSupportOS("windows".to_string()).into()))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cmd::check_env::CheckEnvCmd;
    use crate::cmd::Command;

    #[test]
    pub fn test_os_type() {
        let check_env_cmd = CheckEnvCmd {};

        check_env_cmd.execute();
    }
}