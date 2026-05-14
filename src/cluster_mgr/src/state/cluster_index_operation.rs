use crate::{state_operation_impl, StateValue, Stateful};
use chrono::Utc;
use sqlx::FromRow;

pub(crate) const CLUSTER_INDEX_SELECT: &str = r#"select cluster_name, topology_path, host_list,
                                           create_timestamp, update_timestamp
                                     from  t_cluster_index"#;

pub(crate) const CLUSTER_INDEX_UPSERT: [&str; 2] = [
    r#"insert into t_cluster_index (cluster_name, topology_path, host_list, create_timestamp, update_timestamp) values ("#,
    r#" )on CONFLICT (cluster_name) DO UPDATE SET topology_path = excluded.topology_path,host_list=excluded.host_list,update_timestamp=excluded.update_timestamp"#,
];

pub(crate) const CLUSTER_INDEX_DELETE: &str = r#"delete from t_cluster_index"#;

#[derive(Eq, PartialEq, Clone, Debug, FromRow)]
pub struct ClusterIndexEntity {
    pub cluster_name: String,
    pub topology_path: String,
    pub host_list: String,
    pub create_timestamp: chrono::DateTime<Utc>,
    pub update_timestamp: chrono::DateTime<Utc>,
}

impl Stateful for ClusterIndexEntity {
    fn to_values(&self) -> Vec<StateValue> {
        let self_cloned = self.clone();
        vec![
            StateValue::Varchar(self_cloned.cluster_name),
            StateValue::Varchar(self_cloned.topology_path),
            StateValue::Varchar(self_cloned.host_list),
            StateValue::Timestamp(self_cloned.create_timestamp),
            StateValue::Timestamp(self_cloned.update_timestamp),
        ]
    }
}

state_operation_impl! {
    { ClusterIndexOperation, ClusterIndexEntity, CLUSTER_INDEX_SELECT, CLUSTER_INDEX_UPSERT ,CLUSTER_INDEX_DELETE}
}
