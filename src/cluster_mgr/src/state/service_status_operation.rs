use crate::{state_operation_impl, StateValue, Stateful};
use chrono::Utc;
use sqlx::FromRow;

pub(crate) const SERVICE_STATUS_SELECT: &str = r#"select cluster_name,service_name,service_status,current_config,
                                                  host,create_timestamp,update_timestamp
                                           from t_service_instance "#;

pub(crate) const SERVICE_STATUS_UPSERT: [&str; 2] = [
    r#"insert into t_service_instance(cluster_name,service_name,service_status,current_config,host,create_timestamp,update_timestamp) values( "#,
    r#" )on CONFLICT (cluster_name, service_name) DO UPDATE SET service_status = excluded.service_status,update_timestamp=excluded.update_timestamp"#,
];

pub(crate) const SERVICE_STATUS_DELETE: &str = r#"delete from t_service_instance"#;

#[derive(Eq, PartialEq, Clone, Debug, FromRow)]
pub struct ServiceInstanceEntity {
    cluster_name: String,
    service_name: String,
    service_status: u16,
    current_config: u16,
    host: String,
    create_timestamp: chrono::DateTime<Utc>,
    update_timestamp: chrono::DateTime<Utc>,
}

impl Stateful for ServiceInstanceEntity {
    fn to_values(&self) -> Vec<StateValue> {
        let self_cloned = self.clone();
        vec![
            StateValue::Varchar(self_cloned.cluster_name),
            StateValue::Varchar(self_cloned.service_name),
            StateValue::Integer(self_cloned.service_status as i32),
            StateValue::Integer(self_cloned.current_config as i32),
            StateValue::Varchar(self_cloned.host),
            StateValue::Timestamp(self_cloned.create_timestamp),
            StateValue::Timestamp(self_cloned.update_timestamp),
        ]
    }
}

state_operation_impl! {
    { ServiceInstanceOperation, ServiceInstanceEntity, SERVICE_STATUS_SELECT, SERVICE_STATUS_UPSERT ,SERVICE_STATUS_DELETE}
}
