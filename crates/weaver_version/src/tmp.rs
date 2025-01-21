// SPDX-License-Identifier: Apache-2.0

//! Tmp

use crate::schema_changes::RegistryManifest;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The type of schema item.
#[derive(Debug, Serialize, Hash, Eq, PartialEq, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RegistryItemType {
    /// Attributes
    Attributes,
    /// Metrics
    Metrics,
    /// Events
    Events,
    /// Spans
    Spans,
    /// Resources
    Resources,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegistryChanges {
    /// Information on the registry manifest for the most recent version of the schema.
    head: RegistryManifest,

    /// Information of the registry manifest for the baseline version of the schema.
    baseline: RegistryManifest,

    /// A map where the key is the type of schema item (e.g., "attributes", "metrics",
    /// "events, "spans", "resources"), and the value is a list of changes associated
    /// with that item type.
    changes: HashMap<RegistryItemType, Vec<RegistryItemChange>>,
}

/// This enum represents the different types of changes that can occur
/// between two versions of a registry. This covers changes such as adding,
/// updating, deprecating, and removing registry top-level items such as
/// attributes, resources, metrics, events, and spans.
/// When this change corresponds to an update, the fields and attributes
/// of the item that have been updated are also included.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum RegistryItemChange {
    /// An item (e.g. attribute, resource, metric, ...) has been added
    /// into the most recent version of the registry (also called head
    /// registry).
    Added {
        /// The name of the added item.
        name: String,
    },
    /// An item has been renamed.
    Renamed {
        /// The old name of the item.
        old_name: String,
        /// The new name the item.
        new_name: String,
        
        // ToDo preserve_semantic: bool,
    },
    /// An item has been merged.
    Merged {
        /// The name of the merged item.
        name: String,
        /// The names of the original items that were merged to form the resulting item.
        #[serde(skip_serializing_if = "HashSet::is_empty")]
        source_items: HashSet<String>,
    },
    /// An item has been split.
    Split {
        /// The name of the split item.
        name: String,
        /// The names of the items that have been split.
        #[serde(skip_serializing_if = "HashSet::is_empty")]
        split_into: HashSet<String>,
    },
    /// An item has been updated.
    AttributeUpdated {
        /// The name of the updated item.
        name: String,
        /// A list of fields that have been updated (if any).
        #[serde(skip_serializing_if = "Vec::is_empty")]
        fields: Vec<FieldChange<AttributeFieldName>>,
    },
    /// An item has been updated.
    SignalUpdated {
        /// The name of the updated item.
        name: String,
        /// A list of fields that have been updated (if any).
        #[serde(skip_serializing_if = "Vec::is_empty")]
        fields: Vec<FieldChange<SignalFieldName>>,
        /// A list of attributes that have been updated (if any).
        #[serde(skip_serializing_if = "Vec::is_empty")]
        attributes: Vec<AttributeChange>,
    },
    /// An item has been deprecated.
    Deprecated {
        /// The name of the deprecated item.
        name: String,
        /// A deprecation note providing further context.
        note: String,
    },
    /// An item has been removed.
    Removed {
        /// The name of the removed item.
        name: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalFieldName {
    Brief,
    Note,
    Extends,
    Stability,
    Deprecated,
    Attributes,
    Constraints,
    SpanKind,
    Events,
    MetricName,
    Instrument,
    Unit,
    Name,
    DisplayName,
    Body,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeFieldName {
    Type,
    Brief,
    Examples,
    Tag,
    RequirementLevel,
    SamplingRelevant,
    Note,
    Stability,
    Deprecated,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FieldChange<T> {
    pub name: T,
    pub old_value: String,
    pub new_value: String,
    /// Compatibility information for the change.
    pub compatibility: Compatibility,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    /// The change is backward compatible (older versions can work with newer versions).
    Backward,
    /// The change is forward compatible (newer versions can work with older versions).
    Forward,
    /// The change is both backward and forward compatible.
    Both,
    /// The change is not compatible in either direction.
    None,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeChange {
    Added {
        name: String,
    },
    Updated {
        name: String,
        fields: Vec<FieldChange<AttributeFieldName>>,
    },
    Deprecated {
        name: String,
        note: String,
    },
    Removed {
        name: String,
    },
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use crate::schema_changes::RegistryManifest;
    use crate::tmp::{AttributeFieldName, Compatibility, FieldChange, RegistryChanges, RegistryItemChange, RegistryItemType, SignalFieldName};

    #[test]
    fn test() {
        let mut registry_changes = RegistryChanges {
            head: RegistryManifest { semconv_version: "1.29".to_owned() },
            baseline: RegistryManifest { semconv_version: "1.28".to_owned() },
            changes: HashMap::new(),
        };

        _ = registry_changes.changes.insert(RegistryItemType::Attributes, vec![
            // id: "db.connection_string"
            // deprecated:
            //   action: split
            //   into: ["server.address", "server.port"]
            //   ToDo how to express the backward transformation?
            RegistryItemChange::Split {
                name: "db.connection_string".to_owned(),
                split_into: vec!["server.address".to_owned(), "server.port".to_owned()].into_iter().collect(),
            },
            // id: "db.cassandra.table"
            // deprecated:
            //   action: renamed
            //   renamed_to: "db.collection.name"
            //   ToDo how to express the context used to implement the backward transformation?
            // id: "db.cosmosdb.container"
            // deprecated:
            //   action: renamed
            //   renamed_to: "db.collection.name"
            // id: "db.mongodb.collection"
            // deprecated:
            //   action: renamed
            //   renamed_to: "db.collection.name"
            // id: "db.sql.table"
            // deprecated:
            //   action: renamed
            //   renamed_to: "db.collection.name"
            RegistryItemChange::Merged {
                name: "db.collection.name".to_owned(),
                source_items: vec![
                    "db.cassandra.table".to_owned(),
                    "db.cosmosdb.container".to_owned(),
                    "db.mongodb.collection".to_owned(),
                    "db.sql.table".to_owned(),
                ].into_iter().collect(),
            },
            // id: "db.client.connections.state"
            // deprecated:
            //   action: renamed
            //   renamed_to: "db.client.connection.state"
            RegistryItemChange::Renamed {
                old_name: "db.client.connections.state".to_owned(),
                new_name: "db.client.connection.state".to_owned(),
            },
            // id: "db.jdbc.driver_classname"
            // note: Removed as not used.
            // deprecated:
            //   action: deprecated
            RegistryItemChange::Deprecated {
                name: "db.jdbc.driver_classname".to_owned(),
                note: "Removed as not used.".to_owned(),
            },
            // id: "db.user"
            // note: No replacement at this time.
            // deprecated:
            //   action: deprecated
            RegistryItemChange::Deprecated {
                name: "db.user".to_owned(),
                note: "No replacement at this time.".to_owned(),
            },
            // id: "db.instance.id"
            // note: Deprecated, no general replacement at this time. For Elasticsearch, use `db.elasticsearch.node.name`.
            // deprecated:
            //   action: deprecated
            // ToDo Do we want to support conditional renaming?
            RegistryItemChange::Deprecated {
                name: "db.instance.id".to_owned(),
                note: "Deprecated, no general replacement at this time. For Elasticsearch, use `db.elasticsearch.node.name".to_owned(),
            },
            // # Version 1.28
            // id: "http.status.code"
            // type: string
            // # Version 1.29
            // id: "http.status.code"
            // type: int
            RegistryItemChange::AttributeUpdated {
                name: "http.status.code".to_owned(),
                fields: vec![FieldChange::<AttributeFieldName> {
                    name: AttributeFieldName::Type,
                    old_value: "string".to_string(),
                    new_value: "int".to_string(),
                    compatibility: Compatibility::Both,
                }],
            },
            // id: "db.table.name"
            // note: Deprecated, db.collection.name now represents other entities too (indexes, views, table, etc).
            // deprecated:
            //   action: renamed  # ToDo should we use generalized, expanded, repurposed, unified ?
            //   renamed_to: "db.collection.name"
            //   preserve_semantic: false
            RegistryItemChange::Renamed {
                old_name: "db.table.name".to_owned(),
                new_name: "db.collection.name".to_owned(),
                // ToDo preserve_semantic: false
                // ToDo note: db.collection.name now represents other entities too (indexes, views, table, etc).
            }
        ]);
        _ = registry_changes.changes.insert(RegistryItemType::Metrics, vec![
            // id: "metric.messaging.publish.duration"
            // deprecated:
            //   action: renamed
            //   renamed_to: "messaging.client.operation.duration"
            //   ToDo find a way to express the name of the attribute representing the operation
            // id: "metric.messaging.receive.duration"
            // deprecated:
            //   action: renamed
            //   renamed_to: "messaging.client.operation.duration"
            //   ToDo find a way to express the name of the attribute representing the operation
            RegistryItemChange::Merged {
                name: "messaging.client.operation.duration".to_owned(),
                source_items: vec!["metric.messaging.publish.duration".to_owned(), "metric.messaging.receive.duration".to_owned()].into_iter().collect(),
            },
            // id: "metric.messaging.process.messages"
            // deprecated:
            //   action: renamed
            //   renamed_to: "messaging.client.consumed.messages"
            //   ToDo In this case, is there an attribute that could be used to reverse the renaming?
            // id: "metric.messaging.receive.messages"
            // deprecated:
            //   action: renamed
            //   renamed_to: "messaging.client.consumed.messages"
            //   ToDo In this case, is there an attribute that could be used to reverse the renaming?
            RegistryItemChange::Merged {
                name: "messaging.client.consumed.messages".to_owned(),
                source_items: vec!["metric.messaging.process.messages".to_owned(), "metric.messaging.receive.messages".to_owned()].into_iter().collect(),
            },
            // id: metric.db.client.connections.create_time
            // type: metric
            // deprecated: "Replaced by `db.client.connection.create_time`. Note: the unit also changed from `ms` to `s`."
            RegistryItemChange::SignalUpdated {
                name: "metric.db.client.connections.create_time".to_owned(),
                fields: vec![
                    FieldChange::<SignalFieldName> {
                        name: SignalFieldName::Name,
                        old_value: "metric.db.client.connections.create_time".to_owned(),
                        new_value: "db.client.connection.create_time".to_owned(),
                        compatibility: Compatibility::Both,
                    },
                    FieldChange::<SignalFieldName> {
                        name: SignalFieldName::Unit,
                        old_value: "ms".to_owned(),
                        new_value: "s".to_owned(),
                        compatibility: Compatibility::Both,
                    }
                ],
                attributes: vec![],
            },
            // # Version 1.28
            // id: "http.request"
            // instrument: counter
            // # Version 1.29
            // id: "http.request"
            // instrument: histogram
            RegistryItemChange::SignalUpdated {
                name: "http.request".to_string(),
                fields: vec![
                    FieldChange::<SignalFieldName> {
                        name: SignalFieldName::Instrument,
                        old_value: "counter".to_string(),
                        new_value: "histogram".to_string(),
                        compatibility: Compatibility::None,
                    }
                ],
                attributes: vec![],
            }
        ]);

        let yaml = serde_yaml::to_string(&registry_changes).unwrap();
        println!("{}", yaml);
    }
}

// # Merge
//
// - type: merged
//   name: messaging.client.operation.duration
//   source_items:
//   - metric.messaging.receive.duration
//   - metric.messaging.publish.duration
//   transformation:
//     backward: >
//       switch attr['metric.messaging.operation'] {
//         case `receive` => attr.name = metric.messaging.receive.duration,
//         case `publish` => attr.name = metric.messaging.publish.duration
//       }
//
// - type: updated
//   name: metric.messaging.receive.duration
//   fields:
//   - name: name
//     old_value: metric.messaging.receive.duration
//     new_value: messaging.client.operation.duration
//     condition:
//       backward: attributes['metric.messaging.operation'] == `receive`
//
// - type: updated
//   name: metric.messaging.publish.duration
//   fields:
//   - name: name
//     old_value: metric.messaging.publish.duration
//     new_value: messaging.client.operation.duration
//     condition:
//       backward: attributes['metric.messaging.operation'] == `publish`

// # Split
// 
// - type: updated
//   name: server.address
//   fields:
//   - name: name
//     old_value: db.connection_string
//     new_value: server.address
//     condition:


// ToDo example with the split
// id: "db.connection_string"
// deprecated:
//   action: split
//   into: ["server.address", "server.port"]
//   forward: >
//     attributes['server.address'] = attributes['db.connection_string'].split(':')[0]
//     attributes['server.port'] = attributes['db.connection_string'].split(':')[1]
//   backward: attributes['server.address'] + ':' + attributes['server.port']
//   ToDo how to express the backward transformation?

// Things to think about:
// What the name represent exactly (head or baseline)?
// How to express the backward and forward transformation?