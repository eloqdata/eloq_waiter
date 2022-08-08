use crate::cmd::base::*;
use crate::cmd::cmd_utils::*;
use crate::extract_config_value;
use crate::{build_script, cmd};

#[macro_export]
macro_rules! sync_cmd_impl {
    ($cmd_impl:ident, $cmd_obj:ident, $cmd_enum:ident, $cmd_build_closure:expr) => {
        #[derive(Clone, Debug)]
        pub struct $cmd_impl;

        impl Default for $cmd_impl {
            fn default() -> Self {
                $cmd_impl {}
            }
        }

        impl CmdV2 for $cmd_impl {
            type Executable = $cmd_obj;

            fn definition(&self) -> $cmd_obj {
                $cmd_build_closure()
            }

            fn exec(
                &self,
                context: &mut CmdContext<impl std::io::Write>,
            ) -> Vec<(CmdDef, CmdStatus)> {
                context.run_and_record_context(CmdEnum::$cmd_enum(self.definition()))
            }
        }
    };
}

sync_cmd_impl!(CheckDeps, PipeDef, PipeExec, || { check_deps_as_pipe() });

sync_cmd_impl!(MkdirWorkspace, CmdDef, CmdExec, || {
    use crate::config::{MONOGRAPH_WORKSPACE_DIR, WORKSPACE_LAYOUT};
    let workspace_dir = std::env::var(MONOGRAPH_WORKSPACE_DIR).unwrap();
    let workspace_layout = WORKSPACE_LAYOUT
        .iter()
        .map(|entry| format!("{}/{}", workspace_dir, entry.1))
        .collect::<Vec<_>>();
    let mut cmd_args = vec!["-p".to_string()];
    cmd_args.extend(workspace_layout);

    CmdDef {
        name: "mkdir".to_string(),
        args: Some(cmd_args),
        show_progress_type: None,
        payload: None,
    }
});

sync_cmd_impl!(ExtractTarFile, PipeDef, PipeExec, || {
    use cmd::cmd_const::{CASSANDRA_TAR_FILE_NAME, PROTOBUF_TAR_FILE_NAME};
    let extract_protobuf = extract_tar_cmd(PROTOBUF_TAR_FILE_NAME.to_string());
    let extract_cassandra = extract_tar_cmd(CASSANDRA_TAR_FILE_NAME.to_string());
    PipeDef {
        cmd_vec: vec![extract_protobuf, extract_cassandra],
    }
});

sync_cmd_impl!(LinkMonographSource, CmdDef, CmdExec, || {
    CmdDef {
        name: "bash".to_string(),
        args: Some(vec![
            "-c".to_string(),
            r#"
    #!/bin/bash
    source_dir=${MONOGRAPH_WORKSPACE_DIR}/monograph/source
    monograph_dir=${source_dir}/monograph
    mariadb_dir=${source_dir}/mariadb
    printf "workspace source dir %s \n" ${source_dir}
    printf "workspace monograph source dir %s \n" ${monograph_dir}
    printf "workspace mariadb  source dir %s \n" ${mariadb_dir}
    cd ${mariadb_dir}
    echo "MariaDB git submodule init"
    git_submodel_init="git submodule init"
    eval ${git_submodel_init}
    echo "Link Monograph Source"
    ln -nsF ${source_dir}/log_service ${source_dir}/tx_service/log_service
    ln -nsF ${source_dir}/cass ${monograph_dir}/cass
    ln -nsF ${source_dir}/tx_service ${monograph_dir}/tx_service
    ln -nsF ${monograph_dir} ${mariadb_dir}/storage/monograph
"#
            .to_string(),
        ]),
        show_progress_type: None,
        payload: None,
    }
});

sync_cmd_impl!(ProtobufBuild, PipeDef, PipeExec, || {
    build_script!(download, None, protobuf)
});

sync_cmd_impl!(GitRepoBuild, PipeDef, PipeExec, || {
    build_script!(git, None, brpc, braft, catch2, aws)
});

sync_cmd_impl!(InitMySqlInstance, CmdDef, CmdExec, || {
    init_mysql_instance_cmd()
});
