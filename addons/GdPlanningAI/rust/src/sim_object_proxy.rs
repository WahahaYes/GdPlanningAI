//! Planning-time snapshot of a `GdPAIObjectData` node, built from `get_sim_properties()`.
//!
//! The GDScript-facing form of [`crate::snapshot::SimObjectData`]; main-thread only, since its
//! properties are Godot values.

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
    /// The source node's instance id as a string; the key under which the owning blackboard
    /// stores this proxy.
    pub uid: String,
    /// Names of the groups the source node belonged to when the proxy was created. Fixed for
    /// the proxy's lifetime.
    pub groups: Vec<String>,
    /// Simulation-relevant state, as reported by the source node's `get_sim_properties()` and
    /// then modified by simulated effects.
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
        let group_str = group.to_string();
        self.groups.contains(&group_str)
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

    /// Snapshots a `GdPAIObjectData` node's groups and `get_sim_properties()` into an independent
    /// proxy. Called when a blackboard's `GDPAI_OBJECTS` is set.
    pub fn from_object_data(mut obj: Gd<Node>) -> Option<Gd<SimObjectProxy>> {
        let uid = obj.instance_id().to_i64().to_string();
        let name = obj.get_name().to_string();

        let mut groups = Vec::new();
        let groups_arr = obj.get_groups();
        for g in groups_arr.iter_shared() {
            groups.push(g.to_string());
        }

        let mut properties = HashMap::new();
        match obj
            .call("get_sim_properties", &[])
            .try_to::<VarDictionary>()
        {
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
                    name
                );
            }
        }

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
