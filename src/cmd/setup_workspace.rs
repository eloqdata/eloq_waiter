use crate::cmd::base::{Cmd, CmdStatus};
use crate::cmd::cmd_utils::{cmd_process, elapsed_progress_bar, pipe_progress_bar};
use crate::config::{workspace_sub_dir, MONOGRAPH_WORKSPACE_DIR, WORKSPACE_LAYOUT};
use crate::{extract_config_value, git_clone};
use async_trait::async_trait;
use futures::stream::StreamExt;
use futures_util::future::join_all;
use indicatif::MultiProgress;
use std::fs::File;
use std::io::Write;
use std::path::Path;

static PROTOBUF_TAR_FILE_NAME: &str = "protobuf-bin.tar.gz";
static CASSANDRA_TAR_FILE_NAME: &str = "cassandra-bin.tar.gz";
static LINK_MONOGRAPH_SOURCE: &str = r#"
    #!/bin/bash
    source_dir=${MONOGRAPH_WORKSPACE_DIR}/source
    monograph_dir=${source_dir}/monograph
    mariadb_dir=${source_dir}/mariadb
    cd $mariadb_dir
    echo "MariaDB git submodule init"
    git_submodel_init="git submodule init"
    eval ${git_submodel_init}
    echo "Link Monograph Source"
    ln -s ${monograph_dir} ${mariadb_dir}/storage/monograph
    ln -s ${source_dir}/log_service ${source_dir}/tx_service/log_service
    ln -s ${source_dir}/cass ${monograph_dir}/cass
    ln -s ${source_dir}/tx_service ${monograph_dir}/tx_service
"#;
#[macro_export]
macro_rules! download_task {
    ($multi_progress:expr, $extract_closure:expr) => {{
        let extract_tuple = $extract_closure();
        let pb_m = $multi_progress.clone();
        let task_join = tokio::task::spawn(async move {
            let task_rs = SetupWorkspace::download_async(
                pb_m,
                extract_tuple.0,
                format!("{}", extract_tuple.1),
                format!("{}/{}", extract_tuple.2, "/monograph/third_party"),
            )
            .await;
            if task_rs.is_err() {
                println!("{:?}", task_rs);
            }
        });
        task_join
    }};
}

pub struct SetupWorkspace;

impl SetupWorkspace {
    fn unpack_download_resource(tar_file: String, directory: String) -> CmdStatus {
        let pb = pipe_progress_bar("tar".to_string());
        let cmd_status = cmd_process(
            "tar",
            Some(vec![
                "-zxvf".to_string(),
                tar_file.clone(),
                "-C".to_string(),
                directory,
            ]),
            |std_err| pb.println(std_err),
        );
        pb.finish_with_message(format!("extract {} complete", tar_file));
        cmd_status
    }

    async fn download_third_party() -> CmdStatus {
        let multi_progress = MultiProgress::new();
        let workspace = std::env::var(MONOGRAPH_WORKSPACE_DIR)
            .unwrap_or_else(|_| panic!("MONOGRAPH_WORKSPACE_DIR not set"));

        let protobuf_download_cl = || {
            let common = extract_config_value!("common", Common, None);
            (
                common.clone().compile.download.protobuf.url,
                PROTOBUF_TAR_FILE_NAME,
                workspace.clone(),
            )
        };

        let cassandra_download_cl = || {
            let cassandra = extract_config_value!("cassandra", Storage, None);
            (
                cassandra.clone().download.url,
                CASSANDRA_TAR_FILE_NAME,
                workspace.clone(),
            )
        };
        let join_protobuf = download_task!(multi_progress, protobuf_download_cl);
        let join_cassandra = download_task!(multi_progress, cassandra_download_cl);
        let download_join_all = join_all(vec![join_protobuf, join_cassandra]).await;
        multi_progress.clear().unwrap();
        if download_join_all.is_empty() {
            println!("WARN: Join download task is empty.");
            CmdStatus {
                success: false,
                output: Some("Download task may be failed".to_string()),
            }
        } else {
            println!("Download third_party complete");
            CmdStatus::default()
        }
    }

    async fn download_async(
        multi_progress: MultiProgress,
        resource_url: String,
        download_file_name: String,
        download_dest_path: String,
    ) -> anyhow::Result<()> {
        let rsp_rs = reqwest::get(resource_url.clone()).await;
        if rsp_rs.is_err() {
            return Err(anyhow::Error::from(rsp_rs.err().unwrap()));
        }
        let rsp = rsp_rs.unwrap();
        let download_file_tmp_path =
            Path::new(download_dest_path.as_str()).join(download_file_name.clone());
        let download_file_rs = File::create(download_file_tmp_path.clone());

        if download_file_rs.is_err() {
            return Err(anyhow::Error::from(download_file_rs.err().unwrap()));
        }

        let total_size = rsp
            .content_length()
            .ok_or(format!(
                "Failed to get content length from '{}'",
                resource_url.clone()
            ))
            .unwrap();

        let pb = multi_progress.add(elapsed_progress_bar(
            Some(total_size),
            Some(download_file_name.clone()),
        ));

        let mut download_stream = rsp.bytes_stream();
        let mut download_file = download_file_rs.unwrap();
        let mut downloaded = 0_u64;
        while let Some(stream_chunk) = download_stream.next().await {
            if stream_chunk.is_err() {
                return Err(anyhow::Error::from(stream_chunk.err().unwrap()));
            }
            let chunk_bytes = stream_chunk.unwrap();
            let write_chunks = download_file.write_all(&chunk_bytes);
            if write_chunks.is_err() {
                return Err(anyhow::Error::from(write_chunks.err().unwrap()));
            }
            let new_progress = std::cmp::min(downloaded + (chunk_bytes.len() as u64), total_size);
            downloaded = new_progress;
            pb.set_position(downloaded);
        }
        pb.finish_with_message(format!("{} download compete", download_file_name.clone()));
        Ok(())
    }

    async fn git_clone_all_third_party() -> CmdStatus {
        let common = extract_config_value!("common", Common, None);
        let git = common.clone().compile.git;
        let git_clone_cmd_vec = git_clone!(
            git,
            brpc,
            braft,
            catch2,
            aws,
            tx_service,
            log_service,
            monograph,
            cass,
            mariadb
        );
        let mut join_all_task_vec = Vec::new();
        for git_clone_cmd in git_clone_cmd_vec.clone() {
            let task_handler = tokio::task::spawn(async move {
                let cmd_status = cmd_process(
                    git_clone_cmd.name.clone().as_str(),
                    git_clone_cmd.args,
                    |stdout| {
                        println!("{}", stdout);
                    },
                );
                cmd_status
            });
            join_all_task_vec.push(task_handler);
        }
        let git_clone_all_rs = join_all(join_all_task_vec).await;
        println!("{:?}", git_clone_all_rs);
        let git_cmd_err_count = git_clone_all_rs.iter().filter(|rs| rs.is_err()).count();
        if git_cmd_err_count == 0 {
            CmdStatus::default()
        } else {
            CmdStatus {
                success: false,
                output: Some(format!("git clone repo failure {:?}", git_clone_cmd_vec)),
            }
        }
    }

    fn link_source() -> CmdStatus {
        cmd_process(
            "bash",
            Some(vec![LINK_MONOGRAPH_SOURCE.to_string()]),
            |stdout| println!("{}", stdout),
        )
    }
}

#[async_trait]
impl Cmd for SetupWorkspace {
    fn set_up(&self) -> CmdStatus {
        // MONOGRAPH_WORKSPACE_DIR is set in the main, so there must be
        let workspace_dir = std::env::var(MONOGRAPH_WORKSPACE_DIR).unwrap();
        let workspace_layout = WORKSPACE_LAYOUT
            .iter()
            .map(|entry| format!("{}/{}", workspace_dir, entry.1))
            .collect::<Vec<_>>();
        let mut cmd_args = vec!["-p".to_string()];
        cmd_args.extend(workspace_layout);
        cmd_process("mkdir", Some(cmd_args), |stdout: &str| {
            println!("create workspace {}", stdout);
        })
    }

    async fn exec_async(&self) -> CmdStatus {
        let third_party = workspace_sub_dir().get("third_party").unwrap().clone();
        let download_status = SetupWorkspace::download_third_party().await;
        let status = if download_status.success {
            println!("download task success.");
            let mut tar_status = SetupWorkspace::unpack_download_resource(
                format!("{}/{}", third_party.clone(), PROTOBUF_TAR_FILE_NAME),
                third_party.clone(),
            );
            if tar_status.success {
                tar_status = SetupWorkspace::unpack_download_resource(
                    format!("{}/{}", third_party.clone(), CASSANDRA_TAR_FILE_NAME),
                    third_party.clone(),
                )
            }
            tar_status
        } else {
            download_status
        };
        let git_clone_status = if status.success {
            SetupWorkspace::git_clone_all_third_party().await
        } else {
            status
        };
        if git_clone_status.success {
            SetupWorkspace::link_source()
        } else {
            git_clone_status
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cmd::cmd_utils::cmd_process;
    use crate::cmd::setup_workspace::SetupWorkspace;
    use crate::config::MONOGRAPH_WATER_CONFIG_DIR;
    use std::env;

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_git_clone_cmd() {
        let root = env!("CARGO_MANIFEST_DIR");
        let config_path = format!("{}/{}", root, "config");
        env::set_var(MONOGRAPH_WATER_CONFIG_DIR, config_path);
        let cmd_status = SetupWorkspace::git_clone_all_third_party().await;
        println!("{} {:?}", root, cmd_status);
    }

    #[test]
    pub fn test_cmd_shell() {
        let script = r#"
        echo 'Test ECHO!!!'
        "#;
        cmd_process(
            "bash",
            Some(vec!["-c".to_string(), script.to_string()]),
            |stdout| {
                println!("MY OUT PUT {}", stdout);
            },
        );
    }
}
