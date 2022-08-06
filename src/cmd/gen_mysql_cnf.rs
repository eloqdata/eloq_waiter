use crate::cmd::base::{CmdContext, CmdDef, CmdStatus, CmdV2};
use crate::config::workspace_sub_dir;
use crate::extract_config_value;
use std::io::Write;

const MYSQL_INSTANCE_COUNT: usize = 3;

struct GenMySQLConf;

impl CmdV2 for GenMySQLConf {
    type Executable = CmdDef;

    fn definition(&self) -> CmdDef {
        CmdDef {
            name: "GenMySQLConf".to_string(),
            args: None,
            show_progress_type: None,
            payload: None,
        }
    }

    fn exec(&self, context: &mut CmdContext<impl Write>) -> Vec<(CmdDef, CmdStatus)> {
        let mysql_config = extract_config_value!("mysql", MySQL, None).clone();
        let local_ip = mysql_config.get_from(Some("mariadb"), "monograph_local_ip");
        if local_ip.is_none() {
            let err_msg = "not found config key monograph_local_ip";
            context.logging(err_msg.to_string());
            return self.error_status(err_msg);
        }

        let port_rs = local_ip
            .unwrap()
            .split(';')
            .last()
            .unwrap()
            .parse::<usize>();
        if port_rs.is_err() {
            let err_msg = "monograph_local_ip value error. The format must be IP:PORT";
            context.logging(err_msg.to_string());
            return self.error_status(err_msg);
        }
        let mut port = port_rs.unwrap();
        let workspace_sub_dir = workspace_sub_dir(None);
        let etc_dir = workspace_sub_dir.get("etc").unwrap().clone();
        let data_dir = workspace_sub_dir.get("data").unwrap();
        let install_dir = workspace_sub_dir.get("install").unwrap();

        for idx in 1..=MYSQL_INSTANCE_COUNT {
            port = port * idx + 10;
            let monograph_ip_list = (1..=MYSQL_INSTANCE_COUNT)
                .collect::<Vec<_>>()
                .iter()
                .map(|i| {
                    let my_port = port * i + 10;
                    format!("127.0.0.1:{}", my_port)
                })
                .collect::<Vec<_>>()
                .join(";");

            let mut mysql_cnf_clone = mysql_config.clone();
            mysql_cnf_clone
                .with_section(Some("mariadb"))
                .set("datadir", data_dir)
                .set("lc_messages_dir", format!("{}/share", install_dir))
                .set("monograph_local_ip", format!("127.0.0.1:{}", port))
                .set("monograph_ip_list", monograph_ip_list);

            let config_file = mysql_cnf_clone
                .write_to_file(format!("{}/{}-{}.cnf", etc_dir, "my-conf", idx).as_str());

            if config_file.is_err() {
                let err_msg = config_file.err().unwrap();
                println!("GenMySQLConf Error Cause by{}", err_msg);
                return vec![(
                    CmdDef::default(),
                    CmdStatus {
                        success: false,
                        output: Some(err_msg.to_string()),
                    },
                )];
            }
        }
        vec![(self.definition(), CmdStatus::default())]
    }
}

impl GenMySQLConf {
    fn error_status(&self, err_msg: &str) -> Vec<(CmdDef, CmdStatus)> {
        vec![(
            self.definition(),
            CmdStatus {
                success: false,
                output: Some(err_msg.to_string()),
            },
        )]
    }
}
