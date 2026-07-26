//! Common test utilities for integration tests.

use gdplanningai_rust::snapshot::{BlackboardSnapshot, SimObjectData, VariantSnapshot};
use std::collections::HashMap;

/// Create a test agent snapshot with basic properties.
pub fn create_test_agent(properties: Vec<(&str, VariantSnapshot)>) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    for (key, value) in properties {
        props.insert(key.to_string(), value);
    }
    BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    }
}

/// Create a test world snapshot with objects.
pub fn create_test_world(
    properties: Vec<(&str, VariantSnapshot)>,
    objects: Vec<(&str, SimObjectData)>,
) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    for (key, value) in properties {
        props.insert(key.to_string(), value);
    }

    let mut objs = HashMap::new();
    for (uid, obj) in objects {
        objs.insert(uid.to_string(), obj);
    }

    BlackboardSnapshot {
        properties: props,
        objects: objs,
    }
}

/// Create a simple SimObjectData for testing.
#[allow(dead_code)]
pub fn create_sim_object(
    uid: &str,
    groups: Vec<&str>,
    properties: Vec<(&str, VariantSnapshot)>,
) -> SimObjectData {
    let groups_vec = groups.iter().map(|s| s.to_string()).collect();
    let mut props = HashMap::new();
    for (key, value) in properties {
        props.insert(key.to_string(), value);
    }

    SimObjectData {
        uid: uid.to_string(),
        groups: groups_vec,
        properties: props,
    }
}
