use once_cell::sync::Lazy;
/// Configuration field definitions for the MonographDB system.
///
/// This file defines the available configuration fields that can be updated
/// using the `eloqctl update-conf` command, along with metadata about their
/// update scope (node-specific or cluster-wide) and other properties.
use std::collections::HashMap;

// TODO(ZX) add more fields

/// Represents the scope of a configuration field update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldScope {
    /// Field can be updated on a specific node without affecting others
    NodeSpecific,

    /// Field must be updated across all nodes for consistency
    ClusterWide,
}

/// Holds metadata about a configuration field
#[derive(Debug, Clone)]
pub struct FieldMetadata {
    /// Description of the field's purpose
    pub description: &'static str,

    /// The update scope (node-specific or cluster-wide)
    pub scope: FieldScope,

    /// Example valid value
    pub example: &'static str,

    /// Value type (for validation)
    pub value_type: FieldValueType,

    /// Default value if not specified
    pub default_value: &'static str,
}

/// Represents the type of a configuration field value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldValueType {
    /// String value (path, name, etc.)
    String,

    /// Boolean value (true/false)
    Boolean,

    /// Integer value
    Integer,
}

/// Registry of all available configuration fields with their metadata.
///
/// This allows the update-conf command to validate field names and properly
/// determine which fields require cluster-wide updates.
pub static AVAILABLE_FIELDS: Lazy<HashMap<&'static str, FieldMetadata>> = Lazy::new(|| {
    let mut fields = HashMap::new();

    // Storage configuration fields
    fields.insert(
        "eloq_data_path",
        FieldMetadata {
            description: "Path to the data directory for the EloqKV instance",
            scope: FieldScope::NodeSpecific,
            example: "/home/eloq/my_cluster/EloqKV/data/port-6379",
            value_type: FieldValueType::String,
            default_value: "/home/eloq/{cluster}/EloqKV/data/port-{port}",
        },
    );

    fields.insert(
        "enable_data_store",
        FieldMetadata {
            description: "Whether to enable persistent data storage",
            scope: FieldScope::NodeSpecific,
            example: "true",
            value_type: FieldValueType::Boolean,
            default_value: "true",
        },
    );

    fields.insert(
        "enable_wal",
        FieldMetadata {
            description: "Whether to enable Write Ahead Log for durability",
            scope: FieldScope::NodeSpecific,
            example: "true",
            value_type: FieldValueType::Boolean,
            default_value: "false",
        },
    );

    // Performance tuning fields
    fields.insert(
        "max_connections",
        FieldMetadata {
            description: "Maximum number of client connections",
            scope: FieldScope::NodeSpecific,
            example: "10000",
            value_type: FieldValueType::Integer,
            default_value: "5000",
        },
    );

    fields.insert(
        "timeout",
        FieldMetadata {
            description: "Client connection timeout in seconds",
            scope: FieldScope::NodeSpecific,
            example: "300",
            value_type: FieldValueType::Integer,
            default_value: "60",
        },
    );

    // Cluster configuration fields
    fields.insert(
        "replication_factor",
        FieldMetadata {
            description: "Number of replicas for each master",
            scope: FieldScope::ClusterWide,
            example: "2",
            value_type: FieldValueType::Integer,
            default_value: "1",
        },
    );

    fields.insert(
        "cluster_name",
        FieldMetadata {
            description: "Name of the MonographDB cluster",
            scope: FieldScope::ClusterWide,
            example: "prod-cluster",
            value_type: FieldValueType::String,
            default_value: "default-cluster",
        },
    );

    // Memory management fields
    fields.insert(
        "max_memory",
        FieldMetadata {
            description: "Maximum memory usage in megabytes",
            scope: FieldScope::NodeSpecific,
            example: "4096",
            value_type: FieldValueType::Integer,
            default_value: "2048",
        },
    );

    fields.insert(
        "maxmemory_policy",
        FieldMetadata {
            description: "Policy for memory eviction when max memory is reached",
            scope: FieldScope::NodeSpecific,
            example: "volatile-lru",
            value_type: FieldValueType::String,
            default_value: "noeviction",
        },
    );

    // Security fields
    fields.insert(
        "tls_enabled",
        FieldMetadata {
            description: "Whether to enable TLS encryption",
            scope: FieldScope::ClusterWide,
            example: "true",
            value_type: FieldValueType::Boolean,
            default_value: "false",
        },
    );

    fields.insert(
        "auth_required",
        FieldMetadata {
            description: "Whether authentication is required for connections",
            scope: FieldScope::ClusterWide,
            example: "true",
            value_type: FieldValueType::Boolean,
            default_value: "false",
        },
    );

    fields
});

/// Helper function to check if a field exists in the registry
pub fn field_exists(field_name: &str) -> bool {
    AVAILABLE_FIELDS.contains_key(field_name)
}

/// Helper function to get field metadata if it exists
pub fn get_field_metadata(field_name: &str) -> Option<&FieldMetadata> {
    AVAILABLE_FIELDS.get(field_name)
}

/// Helper function to determine if a field requires a cluster-wide update
pub fn is_cluster_wide_field(field_name: &str) -> bool {
    AVAILABLE_FIELDS
        .get(field_name)
        .map_or(false, |metadata| metadata.scope == FieldScope::ClusterWide)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_exists() {
        assert!(field_exists("eloq_data_path"));
        assert!(field_exists("enable_data_store"));
        assert!(!field_exists("nonexistent_field"));
    }

    #[test]
    fn test_field_scope() {
        assert!(!is_cluster_wide_field("eloq_data_path"));
        assert!(is_cluster_wide_field("replication_factor"));
    }
}
