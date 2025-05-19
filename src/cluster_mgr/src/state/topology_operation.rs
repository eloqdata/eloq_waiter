use crate::{state_operation_impl, StateValue, Stateful};
use chrono::{DateTime, Utc};
use sqlx::FromRow;

pub(crate) const TOPOLOGY_SELECT: &str = r#"select cluster_name, node_group_count, node_group_id, node_id, 
is_candidate, is_master, host, port, cluster_config, create_timestamp, update_timestamp from t_topology"#;

pub(crate) const TOPOLOGY_UPDATE: [&str; 2] = [
    r#"insert into t_topology(cluster_name, node_group_count, node_group_id, node_id, 
is_candidate, is_master, host, port, cluster_config, create_timestamp, update_timestamp) values( "#,
    r#") on conflict(cluster_name, node_group_id, node_id) do update set
    cluster_name=excluded.cluster_name,
    node_group_count=excluded.node_group_count,
    node_group_id=excluded.node_group_id,
    node_id=excluded.node_id,
    is_candidate=excluded.is_candidate,
    is_master=excluded.is_master,
    host=excluded.host,
    port=excluded.port,
    cluster_config=excluded.cluster_config,
    update_timestamp=excluded.update_timestamp
    "#,
];

pub(crate) const TOPOLOGY_DELETE: &str = r#"delete from t_topology"#;

#[derive(Debug, Clone, FromRow)]
pub struct TopologyEntity {
    pub cluster_name: String,
    pub node_group_count: i32,
    pub node_group_id: i32,
    pub node_id: String,
    pub is_candidate: bool,
    pub is_master: bool,
    pub host: String,
    pub port: i32,
    pub cluster_config: Option<String>,
    pub create_timestamp: DateTime<Utc>,
    pub update_timestamp: DateTime<Utc>,
}

impl Stateful for TopologyEntity {
    fn to_values(&self) -> Vec<StateValue> {
        vec![
            StateValue::Varchar(self.cluster_name.clone()),
            StateValue::Integer(self.node_group_count),
            StateValue::Integer(self.node_group_id),
            StateValue::Varchar(self.node_id.clone()),
            StateValue::Integer(self.is_candidate as i32),
            StateValue::Integer(self.is_master as i32),
            StateValue::Varchar(self.host.clone()),
            StateValue::Integer(self.port),
            match &self.cluster_config {
                Some(s) => StateValue::Varchar(s.clone()),
                None => StateValue::Varchar("".to_string()),
            },
            StateValue::Timestamp(self.create_timestamp),
            StateValue::Timestamp(self.update_timestamp),
        ]
    }
}

state_operation_impl! {
    {TopologyOperation, TopologyEntity, TOPOLOGY_SELECT, TOPOLOGY_UPDATE, TOPOLOGY_DELETE}
}
