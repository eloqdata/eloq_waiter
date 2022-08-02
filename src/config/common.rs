use serde_derive::Deserialize;

#[macro_export]
macro_rules! git_clone {
    ($git_obj:expr, $dest_dir:expr $(,$git_attr:ident)*) => {{
        use $crate::cmd::base::CmdDesc;
        let mut cmd_desc_vec: Vec<CmdDesc> = vec![];
        $(
           let mut cmd_desc = CmdDesc::default();
           cmd_desc.name = "git".to_string();
           let mut git_clone_args = vec!["clone".to_string()];
           if let Some(git_option) = $git_obj.$git_attr.options {
               git_clone_args.extend(git_option);
           }
           let dest_name = format!("{}/{}", $dest_dir.to_string(), std::stringify!($git_attr).to_string());
           if let Some(branch_name) = $git_obj.$git_attr.branch {
              git_clone_args.extend(vec!["-b".to_string(), branch_name, $git_obj.$git_attr.git, dest_name]);
           } else {
              git_clone_args.extend(vec![$git_obj.$git_attr.git, dest_name]);
           }
           cmd_desc.args = Some(git_clone_args);
           cmd_desc_vec.push(cmd_desc);
        )*
        cmd_desc_vec
    }};
}
#[derive(Clone, Debug, Deserialize)]
pub struct Common {
    pub workspace: String,
    pub compile: Compile,
    pub monograph: Monograph,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Compile {
    pub download: Download,
    pub git: Git,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Monograph {
    pub storage: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Git {
    pub brpc: GitArgs,
    pub braft: GitArgs,
    pub catch2: GitArgs,
    pub aws: GitArgs,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GitArgs {
    pub git: String,
    pub branch: Option<String>,
    pub build: Option<String>,
    pub options: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Download {
    pub protobuf: DownloadArgs,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DownloadArgs {
    pub url: String,
    pub build: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Cassandra {
    pub download: CassandraDownload,
    pub command: CassandraCommand,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CassandraDownload {
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CassandraCommand {
    pub start_script: String,
}

#[cfg(test)]
mod tests {
    use crate::config::common::{Git, GitArgs};

    #[test]
    pub fn test_git_clone_cmd_macro() {
        let test_git_attr = GitArgs {
            git: "https://github.com/apache/incubator-brpc.git".to_string(),
            branch: Some("v2.x".to_string()),
            build: None,
            options: None,
        };
        let git = Git {
            brpc: test_git_attr.clone(),
            braft: test_git_attr.clone(),
            catch2: test_git_attr.clone(),
            aws: test_git_attr,
        };
        let git_string = stringify!(Git);
        println!("git_string {}", git_string.to_string().to_lowercase());
        let git_cmd = git_clone!(git, "~/Downloads", brpc, braft);
        println!("Cmd {:?}", git_cmd);
        assert_eq!(2, git_cmd.len())
    }
}
