//! Simulation snapshot of a world object used during planning.
//!
//! A [`SimObjectProxy`] captures the state of a `GdPAIObjectData` node at
//! planning start via its `get_sim_properties()` dictionary. Instances live
//! inside a [`super::gdpai_blackboard::GdPAIBlackboard`] and are cloned
//! alongside it when the engine branches into a new simulation path.

use godot::prelude::*;
use std::collections::HashMap;

/// Read/write snapshot of a world object's simulation-relevant state.
///
/// Created from [code]GdPAIObjectData.get_sim_properties()[/code] at the start of
/// planning. Use [method get_property] and [method set_property] to read and
/// mutate state within [code]simulate_effect[/code] or [code]get_action_cost[/code].
/// Changes made to a proxy during simulation are isolated to that branch and
/// never affect the live scene object.
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct SimObjectProxy {
    pub uid: String,
    pub groups: Vec<String>,
    pub properties: HashMap<String, Variant>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for SimObjectProxy {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            uid: String::new(),
            groups: Vec::new(),
            properties: HashMap::new(),
            base,
        }
    }
}

#[godot_api]
impl SimObjectProxy {
    /// Returns [code]true[/code] if this object belongs to [param group].
    #[func]
    pub fn is_in_group(&self, group: GString) -> bool {
        self.groups.contains(&group.to_string())
    }

    /// Returns all groups this object belongs to.
    #[func]
    pub fn get_groups(&self) -> Array<GString> {
        let mut arr = Array::new();
        for g in &self.groups {
            arr.push(&GString::from(g));
        }
        arr
    }

    /// Returns the simulation property stored under [param key], or [code]null[/code] if not set.
    #[func]
    pub fn get_property(&self, key: GString) -> Variant {
        self.properties
            .get(&key.to_string())
            .cloned()
            .unwrap_or(Variant::nil())
    }

    /// Sets the simulation property [param key] to [param value].
    #[func]
    pub fn set_property(&mut self, key: GString, value: Variant) {
        self.properties.insert(key.to_string(), value);
    }

    /// Returns [code]true[/code] if simulation property [param key] is present.
    #[func]
    pub fn has_property(&self, key: GString) -> bool {
        self.properties.contains_key(&key.to_string())
    }

    /// Constructs a [`SimObjectProxy`] from a live `GdPAIObjectData` node.
    ///
    /// Calls `get_groups()` and `get_sim_properties()` on `obj` via dynamic
    /// dispatch to snapshot group membership and simulation-relevant properties.
    /// The resulting proxy is fully independent of the source node.
    pub fn from_object_data(mut obj: Gd<Object>) -> Option<Gd<SimObjectProxy>> {
        let uid = obj.instance_id().to_i64().to_string();

        let mut groups = Vec::new();
        match obj.call("get_groups", &[]).try_to::<Array<StringName>>() {
            Ok(groups_arr) => {
                for g in groups_arr.iter_shared() {
                    groups.push(g.to_string());
                }
            }
            Err(_) => {
                log_warn!(
                    "SimObjectProxy: get_groups() call failed for object {}",
                    uid
                );
            }
        }

        let mut properties = HashMap::new();
        match obj.call("get_sim_properties", &[]).try_to::<VarDictionary>() {
            Ok(sim_props) => {
                for (k, v) in sim_props.iter_shared() {
                    if let Ok(key_str) = k.try_to::<String>() {
                        properties.insert(key_str, v);
                    }
                }
            }
            Err(_) => {
                log_warn!(
                    "SimObjectProxy: get_sim_properties() call failed for object {}",
                    uid
                );
            }
        }

        log_debug!(
            "SimObjectProxy: snapshotted object {} — {} group(s), {} property(-ies)",
            uid,
            groups.len(),
            properties.len()
        );

        let mut proxy = SimObjectProxy::new_gd();
        {
            let mut bind = proxy.bind_mut();
            bind.uid = uid;
            bind.groups = groups;
            bind.properties = properties;
        }

        Some(proxy)
    }
}
