use crate::cmd::base::{CmdContext, CmdDef, CmdStatus, CmdV2};
use crate::cmd::mysql_ctl_util::list_mysql_cnf;
use std::fmt::Debug;
use std::io::Write;
use std::path::Path;
use sysinfo::{PidExt, ProcessExt, ProcessStatus, SystemExt};

#[derive(Clone, Debug)]
pub struct CheckMysqlStatus;

#[derive(Clone, Default, Debug)]
pub struct MySQLProcess {
    pub(crate) pid: u32,
    pub(crate) cmd: Vec<String>,
}

impl MySQLProcess {
    fn extract_cmd_arg(&self, arg_name: &str) -> Vec<String> {
        self.cmd
            .iter()
            .filter(|arg| (*arg).clone().contains(arg_name))
            .cloned()
            .collect::<Vec<String>>()
    }

    pub fn is_monograph_instance(&self, monograph_conf_list: &[String]) -> bool {
        println!("MonographDB config lis = {:?}", monograph_conf_list);
        let exec_cmd = self
            .cmd
            .iter()
            .filter(|arg| (*arg).contains("--defaults-file=")).cloned()
            .collect::<Vec<_>>();
        if exec_cmd.is_empty() {
            false
        } else {
            for arg in exec_cmd {
                if !arg.contains("--defaults-file=") {
                    continue;
                }
                let config_file = arg.replace("--defaults-file=", "");
                let file_name = Path::new(config_file.as_str())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                println!("current mysql process config file {}", file_name);
                if monograph_conf_list.contains(&file_name) {
                    return true;
                }
            }
            false
        }
    }

    pub fn config_file(&self) -> Option<String> {
        let default_files = self.extract_cmd_arg("defaults-file");
        if default_files.is_empty() {
            None
        } else {
            Some(
                default_files
                    .first()
                    .unwrap()
                    .replace("--defaults-file=", ""),
            )
        }
    }
}

impl CmdV2 for CheckMysqlStatus {
    type Executable = CmdDef;
    type StatsData = Vec<MySQLProcess>;

    fn definition(&self) -> CmdDef {
        CmdDef {
            name: "check_mysql_status".to_string(),
            args: None,
            show_progress_type: None,
            payload: None,
        }
    }

    fn exec(
        &self,
        context: &mut CmdContext<impl Write>,
    ) -> Vec<(CmdDef, CmdStatus<Vec<MySQLProcess>>)> {
        let sys = sysinfo::System::new_all();
        let process_list = sys.processes_by_name("mysqld");
        let mut mysql_process_vec: Vec<MySQLProcess> = Vec::new();
        let monograph_conf_list = list_mysql_cnf(None)
            .iter()
            .map(|p| {
                Path::new(p.as_str())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        for process in process_list {
            //println!("{:?}", process);
            if process.status() == ProcessStatus::Zombie {
                continue;
            }
            let pid = process.pid();
            let process_info = MySQLProcess {
                pid: pid.as_u32(),
                cmd: process.cmd().to_vec(),
            };
            if !process_info.is_monograph_instance(&monograph_conf_list) {
                continue;
            }
            context.logging(format!(
                "mysqld pid={:?}/cmd={:?}/cwd={:?}\n",
                pid,
                process.cmd(),
                process.cwd()
            ));
            mysql_process_vec.push(process_info);
        }
        println!("found mysql process={:#?}", mysql_process_vec);
        vec![(
            self.definition(),
            CmdStatus {
                success: true,
                output: None,
                data: Some(mysql_process_vec),
            },
        )]
    }
}

#[cfg(test)]
mod tests {
    use crate::cmd::base::{CmdContext, CmdV2};
    use crate::cmd::check_mysql_status::CheckMysqlStatus;
    use crate::config::{MONOGRAPH_WATER_CONFIG_DIR, MONOGRAPH_WORKSPACE_DIR};

    #[test]
    pub fn test_diff_config() {
        let mut mysql_config_list = vec![
            "/test_workspace//monograph/etc/my-conf-3317.cnf".to_string(),
            "/test_workspace//monograph/etc/my-conf-3318.cnf".to_string(),
            "/test_workspace//monograph/etc/my-conf-3319.cnf".to_string(),
        ];

        let curr_list = vec![
            "/test_workspace//monograph/etc/my-conf-3317.cnf".to_string(),
            "/test_workspace//monograph/etc/my-conf-3318.cnf".to_string(),
            "/test_workspace//monograph/etc/my-conf-3319.cnf".to_string(),
        ];

        for process in &curr_list {
            if mysql_config_list.is_empty() {
                break;
            }
            mysql_config_list.retain(|x| x != process);
        }

        println!("{:?}", mysql_config_list);
    }

    #[test]
    pub fn test_mysql_process() {
        std::env::set_var(MONOGRAPH_WORKSPACE_DIR, "/test_workspace");
        std::env::set_var(
            MONOGRAPH_WATER_CONFIG_DIR,
            "/home/mono/monograph_waiter/HH3JxhQgv2/config",
        );
        let mut context = CmdContext::new(std::io::stdout());
        let check_mysql_status = CheckMysqlStatus {};
        let cmd_status = check_mysql_status.exec(&mut context);
        println!("{:?}", cmd_status);
        assert!(!cmd_status.is_empty());
        let (_, check_status) = cmd_status.get(0).unwrap();
        println!("mono graph instance = {:?}", check_status);

        //let process_list = check_status.data.clone().unwrap();
        //let mysql_cnf_list = list_mysql_cnf(None);
    }
}
