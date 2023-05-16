use crate::cli::download_dir;
use std::collections::HashMap;

use crate::config::config_base::MONOGRAPH_TX_SERVICE_DIR;
use crate::config::monitor::Monitor;
use crate::config::storage_service_config::StorageService;
use crate::config::{
    config_template, CONFIG_MARIADB_SECTION, MONOGRAPH_CONF_DYNAMO_TEMPLATE,
    MONOGRAPH_CONF_TEMPLATE,
};
use anyhow::anyhow;
use configparser::ini::Ini;
use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const LOG_SRV_REPLICA_NUM: usize = 3;

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct LogProcessKey {
    pub host: String,
    pub port: u16,
}

impl ToString for LogProcessKey {
    fn to_string(&self) -> String {
        let port = self.port;
        let host = &self.host;
        format!("{host}:{port}")
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct LogCmdItems {
    pub group_members_config: String,
    pub log_member: LogGroupMember,
}

#[derive(PartialEq, Debug, Clone)]
pub struct LogGroupMember {
    pub node_id: usize,
    pub group_id: usize,
    pub member_host: String,
    pub port: u16,
    pub storage_path: String,
    pub check_health_url: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Port {
    pub mysql_port: u16,
    pub monograph_port: MonographPort,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MonographPort {
    pub start: u16,
    pub end: u16,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MonographService {
    pub host: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LogService {
    pub nodes: Vec<LogServiceNode>,
    pub replica: Option<u32>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LogServiceNode {
    pub host: String,
    pub data_dir: Vec<String>,
    pub port: u16,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Deployment {
    pub tx_image: String,
    pub log_image: Option<String>,
    pub cluster_name: String,
    pub install_dir: String,
    pub port: Port,
    pub tx_service: MonographService,
    pub log_service: Option<LogService>,
    pub storage_service: StorageService,
    pub monitor: Option<Monitor>,
}

impl LogService {
    pub fn log_host_unique(&self) -> Vec<String> {
        self.nodes
            .iter()
            .map(|log_node| log_node.host.clone())
            .unique()
            .collect_vec()
    }

    fn group(&self) -> usize {
        let host_num = self.log_host_unique().len();
        let group_member = self.log_replica();
        if host_num < group_member {
            1_usize
        } else {
            let group = host_num % group_member;
            if group == 0 {
                host_num / group_member
            } else {
                (host_num / group_member) + 1
            }
        }
    }

    fn gen_log_node_ids(&self, start: usize) -> Vec<usize> {
        let node_len = self.nodes.len();
        let from = if start > node_len { 0 } else { start };
        (0..self.log_replica())
            .into_iter()
            .map(|idx| {
                if from + idx > node_len - 1 {
                    from + idx - node_len
                } else {
                    from + idx
                }
            })
            .collect_vec()
    }

    pub fn group_members(&self) -> HashMap<usize, Vec<LogGroupMember>> {
        let group = self.group();
        let replica = self.log_replica();
        let mut start = 0;
        let mut port_usage = HashMap::new();
        (0..group)
            .into_iter()
            .map(|group_id| {
                let node_ids = self.gen_log_node_ids(start);
                // println!("node_ids={node_ids:?}");
                let members = node_ids
                    .iter()
                    .enumerate()
                    .map(|(idx, id)| {
                        let node = self.nodes.get(*id).unwrap();
                        let port = if !port_usage.contains_key(&node.host) {
                            port_usage.insert(node.host.clone(), node.port);
                            node.port
                        } else {
                            *port_usage.get(&node.host).unwrap() + (group_id + idx) as u16
                        };

                        let data_dir_len = node.data_dir.len();
                        let data_dir = if *id >= data_dir_len {
                            node.data_dir.last().unwrap()
                        } else {
                            node.data_dir.get(*id).unwrap()
                        };
                        let node_host = node.host.clone();
                        LogGroupMember {
                            node_id: idx,
                            group_id,
                            member_host: node_host.clone(),
                            port,
                            storage_path: format!("{data_dir}/group_{group_id}_node_{idx}"),
                            check_health_url: format!("{node_host}:{port}/healthz"),
                        }
                    })
                    .collect_vec();
                start += replica;
                (group_id, members)
            })
            .collect::<HashMap<usize, Vec<LogGroupMember>>>()
    }

    fn log_replica(&self) -> usize {
        if let Some(replica_num) = self.replica {
            replica_num as usize
        } else {
            LOG_SRV_REPLICA_NUM
        }
    }

    fn group_member_config(&self, members: &[LogGroupMember]) -> IndexMap<usize, String> {
        let group_members = members
            .iter()
            .into_group_map_by(|log_member| log_member.group_id);

        group_members
            .into_iter()
            .map(|(group_id, inner_group_member)| {
                let member_config = inner_group_member
                    .iter()
                    .map(|inner_member| {
                        format!("{}:{}", inner_member.member_host, inner_member.port)
                    })
                    .collect_vec()
                    .join(",");

                (group_id, member_config)
            })
            .collect::<IndexMap<usize, String>>()
    }

    /// Grouping by host means categorizing log member processes on that node.
    fn host_members(&self, all_members: &[LogGroupMember]) -> HashMap<String, Vec<LogGroupMember>> {
        all_members
            .iter()
            .into_group_map_by(|log_member| log_member.member_host.clone())
            .into_iter()
            .map(|(host, members)| (host, members.into_iter().cloned().collect()))
            .collect()
    }

    fn group_member_as_vec(&self) -> Vec<LogGroupMember> {
        self.group_members()
            .values()
            .flat_map(|val| val.iter().cloned().collect_vec())
            .collect_vec()
    }

    /// log startup command, with host as granularity, key is hostname value is start command.
    pub fn log_start_cmd(&self) -> HashMap<String, Vec<LogCmdItems>> {
        let all_member_vec = self.group_member_as_vec(); //self.group_members();
        let all_member_as_slice = all_member_vec.as_slice();
        let group_member_config = self.group_member_config(all_member_as_slice);
        let host_members_lookup = self.host_members(all_member_as_slice);

        host_members_lookup
            .iter()
            .map(|(host, members)| {
                let cmds = members
                    .iter()
                    .map(|log_member| {
                        let group_id = log_member.group_id;
                        let member_config = group_member_config.get(&group_id).unwrap().clone();
                        LogCmdItems {
                            group_members_config: member_config,
                            log_member: log_member.clone(),
                        }
                    })
                    .collect_vec();
                (host.to_string(), cmds)
            })
            .collect::<HashMap<String, Vec<LogCmdItems>>>()
    }
}

impl Deployment {
    fn build_log_config(&self) -> Option<HashMap<String, String>> {
        if let Some(ref log_srv) = self.log_service {
            let replica_num = log_srv.log_replica();
            let all_members = log_srv.group_member_as_vec();
            let group_member_map = log_srv.group_member_config(all_members.as_slice());
            let node_group = Vec::from_iter(group_member_map.values())
                .into_iter()
                .join(",");
            Some(HashMap::from([
                (
                    "monograph_txlog_group_replica_num".to_string(),
                    replica_num.to_string(),
                ),
                ("monograph_txlog_service_list".to_string(), node_group),
            ]))
        } else {
            None
        }
    }

    pub fn bootstrap_host(&self) -> String {
        let mut all_hosts = self.tx_service.host.clone();
        assert!(!all_hosts.is_empty());
        all_hosts.sort();
        all_hosts.first().unwrap().to_string()
    }

    pub fn build_monograph_config(
        &self,
        set_ip_list: bool,
        install_dir: String,
    ) -> anyhow::Result<Ini> {
        let mut mysql_ini = Ini::new();
        if let Some(cassandra) = self.storage_service.cassandra.as_ref() {
            mysql_ini
                .load(config_template(MONOGRAPH_CONF_TEMPLATE)?.as_path())
                .unwrap();

            let cassandra_hosts = cassandra.host.join(",");
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_cass_hosts",
                Some(cassandra_hosts),
            );
        } else {
            mysql_ini
                .load(config_template(MONOGRAPH_CONF_DYNAMO_TEMPLATE)?.as_path())
                .unwrap();

            let dynamodb = self.storage_service.dynamodb.as_ref().unwrap();
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_aws_access_key_id",
                Some(dynamodb.clone().access_key_id),
            );
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_aws_secret_key",
                Some(dynamodb.clone().secret_key),
            );
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_dynamodb_region",
                Some(dynamodb.clone().region),
            );
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_dynamodb_endpoint",
                Some(dynamodb.clone().endpoint),
            );
        }

        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "datadir",
            Some(format!("{install_dir}/datafarm")),
        );
        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "lc_messages_dir",
            Some(format!(
                "{install_dir}/{MONOGRAPH_TX_SERVICE_DIR}/install/share"
            )),
        );
        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "plugin_dir",
            Some(format!(
                "{install_dir}/{MONOGRAPH_TX_SERVICE_DIR}/install/lib/plugin",
            )),
        );
        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "port",
            Some(self.port.mysql_port.to_string()),
        );

        mysql_ini.set(
            CONFIG_MARIADB_SECTION,
            "socket",
            Some(format!("/tmp/mysql{}.sock", self.port.mysql_port)),
        );

        let use_port = self.port.monograph_port.start;
        let monograph_hosts = &self.tx_service.host;
        if set_ip_list {
            let ip_list = monograph_hosts
                .iter()
                .map(|host| format!("{}:{}", host.clone(), use_port))
                .join(",");
            mysql_ini.set(CONFIG_MARIADB_SECTION, "monograph_ip_list", Some(ip_list));
        } else {
            mysql_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_ip_list",
                Some(format!("{}:{}", "127.0.0.1", use_port)),
            );
        }
        Ok(mysql_ini.clone())
    }

    pub fn gen_monograph_config(
        &self,
        tx_host: Option<String>,
        install_dir: String,
    ) -> anyhow::Result<PathBuf> {
        let port = self.port.monograph_port.start;
        let set_ip_list = tx_host.is_some();
        let my_ini_rs = self.build_monograph_config(set_ip_list, install_dir);

        let host_and_file_tuple = if let Some(host) = tx_host {
            (host.clone(), host)
        } else {
            ("127.0.0.1".to_string(), "local".to_string())
        };
        let file_suffix = host_and_file_tuple.1;
        let db_config_location = download_dir().join(format!("my_{file_suffix}.cnf"));
        let log_member_config = self.build_log_config();
        if let Ok(mut my_ini) = my_ini_rs {
            if !file_suffix.eq("local") {
                if let Some(config_map) = log_member_config {
                    config_map.iter().for_each(|(key, conf_val)| {
                        my_ini.set(CONFIG_MARIADB_SECTION, key, Some(conf_val.to_string()));
                    });
                }
            }
            my_ini.set(
                CONFIG_MARIADB_SECTION,
                "monograph_local_ip",
                Some(format!("{}:{}", host_and_file_tuple.0, port)),
            );

            if let Err(err) = my_ini.write(db_config_location.clone()) {
                Err(anyhow!(err))
            } else {
                Ok(db_config_location)
            }
        } else {
            Err(my_ini_rs.err().unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::deployment::{LogService, LogServiceNode};
    use itertools::Itertools;

    fn mock_log_service(host_num: usize, replica: usize) -> LogService {
        let nodes = (0..host_num)
            .into_iter()
            .map(|idx| LogServiceNode {
                host: format!("127.0.0.{idx}"),
                data_dir: vec!["/data/opt/log_srv".to_string()],
                port: 9800,
            })
            .collect_vec();
        LogService {
            nodes,
            replica: Some(replica as u32),
        }
    }

    #[test]
    pub fn test_log_service_groups() {
        let one_host_log_srv = &mock_log_service(1, 3);
        let group = one_host_log_srv.group();
        println!("host=1,group_size={group}");
        assert_eq!(1, group);

        let multi_host_log_srv = &mock_log_service(4, 3);
        let group = multi_host_log_srv.group();
        println!("host=4,group_size={group}");
        assert_eq!(2, group);
    }

    #[test]
    pub fn test_log_group_members() {
        let log_srv = &mock_log_service(4, 3);
        let expect_group = 2;
        let expect_members = 2 * log_srv.log_replica();
        let members = log_srv.group_members();
        println!("log_members={members:#?}");
        let groups = members
            .iter()
            .flat_map(|(_, member)| member.iter().map(|inner_member| inner_member.group_id))
            .unique()
            .count();
        println!("groups={groups}");
        assert_eq!(expect_members, members.len());
        assert_eq!(expect_group, groups);
    }

    #[test]
    pub fn test_log_start_cmd() {
        let log_srv = &mock_log_service(5, 3);
        let log_srv_cmd = log_srv.log_start_cmd();
        println!("log_srv_cmd={log_srv_cmd:#?}");
        let hosts = log_srv_cmd.keys();
        assert_eq!(5, hosts.len());
    }

    #[test]
    pub fn test_group_member_config() {
        let log_srv = &mock_log_service(4, 3);
        let binding = log_srv.group_member_as_vec();
        let all_members = binding.as_slice();
        let group_member_config = log_srv.group_member_config(all_members);
        println!("{group_member_config:#?}");
        assert_eq!(2, group_member_config.len());
        let all_config = Vec::from_iter(group_member_config.values())
            .into_iter()
            .join(",");
        println!("all_config={all_config}");
        let config_split = all_config.split(',').count();
        let item_members_count = 2 * log_srv.log_replica();
        assert_eq!(item_members_count, config_split);
    }
}
