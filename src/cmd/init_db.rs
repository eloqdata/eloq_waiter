use crate::cmd::base::{CmdContext, CmdDef, CmdStatus, CmdV2};
use crate::cmd::cmd_macro::{CopySchemData, InitMySQLInstance, MkDataDir};
use crate::cmd::cmd_utils::cmd_status_ok;
use std::io::Write;

pub struct InitDB;

impl InitDB {
    pub fn exec(&self, context: &mut CmdContext<impl Write>) -> Vec<(CmdDef, CmdStatus)> {
        let init_db_instance = InitMySQLInstance {};
        let db_instance_status = init_db_instance.exec(context);
        if !cmd_status_ok(&db_instance_status) {
            return db_instance_status;
        }
        let mk_data_dir_status = MkDataDir {}.exec(context);
        if !cmd_status_ok(&mk_data_dir_status) {
            return mk_data_dir_status;
        }
        let copy_schema_data = CopySchemData {}.exec(context);
        if !cmd_status_ok(&copy_schema_data) {
            return copy_schema_data;
        }

        vec![
            &db_instance_status[..],
            &mk_data_dir_status[..],
            &copy_schema_data[..],
        ]
        .concat()
    }
}
