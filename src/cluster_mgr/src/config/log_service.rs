use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const LOG_SRV_REPLICA_NUM: usize = 3;

#[derive(PartialEq, Eq, Debug, Clone)]
struct NodeDiskIndexPair {
    host_idx: i32,
    host: String,
    dist_idx: i32,
    disk: String,
}

impl Default for NodeDiskIndexPair {
    fn default() -> Self {
        Self {
            host_idx: i32::MIN,
            host: "_NONE_HOST_".to_string(),
            dist_idx: i32::MIN,
            disk: "_NONE_DISK_".to_string(),
        }
    }
}

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

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct LogCmdItems {
    pub group_members_config: String,
    pub log_member: LogGroupMember,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct LogGroupMember {
    pub node_id: usize,
    pub group_id: usize,
    pub member_host: String,
    pub port: u16,
    pub storage_path: String,
    pub check_health_url: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LogServiceNode {
    pub host: String,
    pub data_dir: Vec<String>,
    pub port: u16,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LogService {
    pub nodes: Vec<LogServiceNode>,
    pub replica: Option<u32>,
    pub readiness: Option<LogReadiness>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LogReadiness {
    pub timeout_sec: u64,
    pub delay_ms: Option<usize>,
    pub success_threshold: Option<usize>,
}

impl Default for LogReadiness {
    fn default() -> Self {
        Self {
            timeout_sec: 300,
            delay_ms: None,
            success_threshold: Some(3),
        }
    }
}

impl LogService {
    pub fn log_host_unique(&self) -> Vec<String> {
        self.nodes
            .iter()
            .map(|log_node| log_node.host.clone())
            .unique()
            .collect_vec()
    }

    pub fn readiness_opts(&self) -> LogReadiness {
        if let Some(readiness_ref) = self.readiness.as_ref() {
            readiness_ref.clone()
        } else {
            LogReadiness::default()
        }
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
                    node_len - 1
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
        // println!("The logService contains [{group:?}] group");
        (0..group)
            .into_iter()
            .map(|group_id| {
                let node_ids = self.gen_log_node_ids(start);
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
                            check_health_url: format!("http://{node_host}:{port}/healthz"),
                        }
                    })
                    .collect_vec();
                start += replica;
                (group_id, members)
            })
            .collect::<HashMap<usize, Vec<LogGroupMember>>>()
    }

    pub fn log_replica(&self) -> usize {
        if let Some(replica_num) = self.replica {
            replica_num as usize
        } else {
            LOG_SRV_REPLICA_NUM
        }
    }

    fn node_and_disk_matrix(&self) -> Vec<Vec<NodeDiskIndexPair>> {
        let mut node_sorted = self.nodes.clone();
        node_sorted.sort_by_key(|node| node.host.clone());
        let cols = self.nodes.len();
        let rows = self
            .nodes
            .iter()
            .map(|node| node.data_dir.len())
            .max()
            .unwrap();

        let mut matrix = vec![vec![NodeDiskIndexPair::default(); cols]; rows];
        for (row, item) in matrix.iter_mut().enumerate().take(rows) {
            node_sorted.iter().enumerate().for_each(|(node_idx, node)| {
                let storage = &node.data_dir;
                let host = &node.host;
                let disk_len = storage.len();
                if row < disk_len {
                    let disk = storage.get(row).unwrap();
                    item[node_idx] = NodeDiskIndexPair {
                        host_idx: node_idx as i32,
                        host: host.to_string(),
                        dist_idx: row as i32,
                        disk: disk.to_string(),
                    };
                };
            });
        }
        matrix
    }

    /// The log_group distribution strategy ensures maximum disk utilization
    /// while maintaining availability for the io-sensitive application, log_service.
    /// There are two aspects to this strategy:
    /// 1. members within a log group are distributed across different machines to balance
    ///    the number of leaders on each node;
    /// 2. to maximize throughput, all disks are utilized as much as possible by ensuring
    ///    uniform distribution of disks across nodes. This ensures balanced load distribution among nodes.
    fn memberships(&self) -> HashMap<usize, Vec<LogGroupMember>> {
        let mut node_sorted = self.nodes.clone();
        node_sorted.sort_by_key(|node| node.host.clone());
        let node_host_matrix = self.node_and_disk_matrix();
        let member_count = self.log_replica();
        let mut members = HashMap::new();
        let mut group_id = 0_usize;
        let mut port_usage: HashMap<String, u16> = HashMap::default();
        for node_host_r in &node_host_matrix {
            let mut member_idx = 0_usize;
            let mut member_collect = vec![];
            let len = node_host_r.len();
            println!("memberships  cols ={len}");
            for node_host_l in node_host_r {
                if node_host_l.clone().eq(&NodeDiskIndexPair::default()) {
                    return members;
                }
                let node_idx = node_host_l.host_idx;
                let node = node_sorted.get(node_idx as usize).unwrap();
                let curr_port = node.port;

                let member_host = &node_host_l.host;
                let member_storage = &node_host_l.disk;

                let port = if !port_usage.contains_key(member_host) {
                    port_usage.insert(member_host.clone(), curr_port);
                    node.port
                } else {
                    *port_usage.get(&node.host).unwrap() + (group_id + member_idx) as u16
                };
                let group_member_obj = LogGroupMember {
                    node_id: member_idx,
                    group_id,
                    member_host: member_host.to_string(),
                    port,
                    storage_path: member_storage.to_string(),
                    check_health_url: format!("http://{member_host}:{port}/healthz"),
                };
                member_collect.push(group_member_obj);
                if member_idx < member_count {
                    println!("next_member curr_node={member_idx} {group_id}");
                    member_idx += 1;
                } else {
                    member_idx = 0;
                    group_id += 1;
                }
            }
            members.insert(group_id, member_collect);
            group_id += 1;
        }
        return members;
    }

    pub fn group_member_config(&self, members: &[LogGroupMember]) -> IndexMap<usize, String> {
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

    pub fn group_member_as_vec(&self) -> Vec<LogGroupMember> {
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

#[cfg(test)]
mod tests {
    use crate::config::log_service::{LogReadiness, LogService, LogServiceNode};
    use itertools::Itertools;

    fn mock_log_service(host_num: usize, replica: usize, disks: usize) -> LogService {
        let disk_path = (0..disks)
            .into_iter()
            .map(|disk_idx| format!("/data/opt/disk_{disk_idx}"))
            .collect_vec();

        let nodes = (0..host_num)
            .into_iter()
            .map(|idx| LogServiceNode {
                host: format!("127.0.0.{idx}"),
                data_dir: disk_path.clone(),
                port: 9400,
            })
            .collect_vec();
        LogService {
            nodes,
            replica: Some(replica as u32),
            readiness: Some(LogReadiness::default()),
        }
    }

    #[test]
    pub fn test_gen_nodes() {
        let one_host_log_srv = &mock_log_service(1, 3, 3);
        let nodes = one_host_log_srv.gen_log_node_ids(0);
        println!("{nodes:?}");
        let expected_total_nodes = nodes.iter().sum::<usize>();
        assert_eq!(0, expected_total_nodes);
    }

    #[test]
    pub fn test_log_service_groups() {
        let one_host_log_srv = &mock_log_service(1, 3, 3);
        let group = one_host_log_srv.group();
        println!("host=1,group_size={group}");
        assert_eq!(1, group);

        let multi_host_log_srv = &mock_log_service(4, 3, 3);
        let group = multi_host_log_srv.group();
        println!("host=4,group_size={group}");
        assert_eq!(2, group);
    }

    #[test]
    pub fn test_log_group_members() {
        let log_srv = &mock_log_service(4, 3, 3);
        let matrix = log_srv.node_and_disk_matrix();
        let rows = matrix.len();
        let cols = matrix.get(0).unwrap().len();
        assert_eq!(3, rows);
        assert_eq!(4, cols);
        let groups = (rows * cols) / log_srv.log_replica();
        let members = log_srv.memberships();
        println!("members = {members:#?}");
        // assert_eq!(groups, members.len());
    }

    #[test]
    pub fn test_log_start_cmd() {
        let log_srv = &mock_log_service(5, 3, 3);
        let log_srv_cmd = log_srv.log_start_cmd();
        println!("log_srv_cmd={log_srv_cmd:#?}");
        let hosts = log_srv_cmd.keys();
        assert_eq!(5, hosts.len());
    }

    #[test]
    pub fn test_group_member_config() {
        let log_srv = &mock_log_service(4, 3, 3);
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
