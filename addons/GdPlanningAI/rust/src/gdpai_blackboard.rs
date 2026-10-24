//! Blackboard that holds agent and world state during planning.
//!
//! A [`GdPAIBlackboard`] stores named scalar properties and a map of
//! [`SimObjectProxy`] objects. [`GdPAIBlackboard::clone_for_simulation`] makes an independent
//! copy that holds proxies only.
//!
//! This is the main-thread, GDScript-facing form of state. The planner thread never sees it:
//! [`crate::snapshot::BlackboardSnapshot::from_blackboard`] copies it into Send-safe data
//! before planning, and `into_blackboard` rebuilds one when a GDScript callable needs it.

use crate::sim_object_proxy::SimObjectProxy;
use godot::prelude::*;
use std::collections::HashMap;

/// Key/value store for agent or world state, accessible from GDScript.
///
/// Holds [b]properties[/b] ([method get_property], [method set_property]) and world [b]objects[/b]
/// under [code]GDPAI_OBJECTS[/code], queried by group: [method get_proxy_in_group] returns
/// [SimObjectProxy] snapshots during planning, [method get_node_in_group] live nodes during
/// execution. Blackboards rebuilt for planning callbacks hold proxies only.
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GdPAIBlackboard {
    /// All named properties, including the raw `GDPAI_OBJECTS` array if one was set.
    pub properties: HashMap<String, Variant>,
    /// Planning snapshots of the world objects, keyed by uid (instance id as a string).
    /// Rebuilt whenever `GDPAI_OBJECTS` is set; copied on `clone_for_simulation`.
    pub objects: HashMap<String, Gd<SimObjectProxy>>,
    /// Live nodes behind `objects`. Empty on simulation blackboards, so planning code cannot
    /// reach live nodes.
    pub source_objects: HashMap<String, Gd<Node>>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for GdPAIBlackboard {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            properties: HashMap::new(),
            objects: HashMap::new(),
            source_objects: HashMap::new(),
            base,
        }
    }
}

#[godot_api]
impl GdPAIBlackboard {
    /// Returns the value stored under [param key], or [code]null[/code] if the key is not present.
    #[func]
    pub fn get_property(&self, key: GString) -> Variant {
        self.properties
            .get(&key.to_string())
            .cloned()
            .unwrap_or(Variant::nil())
    }

    /// Sets [param key] to [param value].
    ///
    /// Setting the reserved key [code]GDPAI_OBJECTS[/code] with an [code]Array[/code] of
    /// [code]GdPAIObjectData[/code] nodes also rebuilds the internal object map.
    #[func]
    pub fn set_property(&mut self, key: GString, value: Variant) {
        let key_str = key.to_string();
        if key_str == "GDPAI_OBJECTS" {
            self.objects.clear();
            self.source_objects.clear();
            if let Ok(objects_array) = value.clone().try_to::<Array<Variant>>() {
                for obj_var in objects_array.iter_shared() {
                    match obj_var.try_to::<Gd<Node>>() {
                        Ok(node_gd) => {
                            if let Some(sim_obj) =
                                crate::sim_object_proxy::SimObjectProxy::from_object_data(
                                    node_gd.clone(),
                                )
                            {
                                let uid = sim_obj.bind().uid.clone();
                                self.source_objects.insert(uid.clone(), node_gd);
                                self.objects.insert(uid, sim_obj);
                            }
                        }
                        Err(_) => {
                            log_debug!("GDPAI_OBJECTS: skipping item that is not a Node");
                        }
                    }
                }
            }
        }
        self.properties.insert(key_str, value);
    }

    /// Returns [code]true[/code] if [param key] is present in this blackboard.
    #[func]
    pub fn has_property(&self, key: GString) -> bool {
        self.properties.contains_key(&key.to_string())
    }

    /// Removes [param key] and its value.
    ///
    /// Erasing [code]GDPAI_OBJECTS[/code] also clears the internal object map.
    #[func]
    pub fn erase_property(&mut self, key: GString) {
        let key_str = key.to_string();
        if key_str == "GDPAI_OBJECTS" {
            self.objects.clear();
            self.source_objects.clear();
        }
        self.properties.remove(&key_str);
    }

    /// Returns all properties as a [Dictionary].
    #[func]
    pub fn get_dict(&self) -> VarDictionary {
        let mut dict = VarDictionary::new();
        for (k, v) in &self.properties {
            let _ = dict.insert(k.clone(), v.clone());
        }
        dict
    }

    /// Replaces all properties with those from [param dict].
    ///
    /// Equivalent to clearing the blackboard and calling [method set_property] for each entry.
    #[func]
    pub fn set_dict(&mut self, dict: VarDictionary) {
        self.properties.clear();
        self.objects.clear();
        self.source_objects.clear();
        for (k, v) in dict.iter_shared() {
            if let Ok(key) = k.try_to::<GString>() {
                self.set_property(key, v);
            }
        }
    }

    /// Returns all [SimObjectProxy] planning snapshots that belong to [param group].
    /// Use during planning (cost calculation, effect simulation, precondition checks).
    #[func]
    pub fn get_proxies_in_group(&self, group: GString) -> Array<Gd<SimObjectProxy>> {
        let mut arr = Array::new();
        for obj in self.objects.values() {
            if obj.bind().is_in_group(group.clone()) {
                arr.push(obj);
            }
        }
        arr
    }

    /// Returns the first [SimObjectProxy] planning snapshot belonging to [param group],
    /// or [code]null[/code] if none exists.
    /// Use during planning (cost calculation, effect simulation, precondition checks).
    #[func]
    pub fn get_proxy_in_group(&self, group: GString) -> Variant {
        for obj in self.objects.values() {
            if obj.bind().is_in_group(group.clone()) {
                return obj.clone().to_variant();
            }
        }
        Variant::nil()
    }

    /// Returns the live source node belonging to [param group], or [code]null[/code] if none.
    /// Use during action execution when you need the real, mutable node — not a planning snapshot.
    /// Returns [code]null[/code] on simulation-cloned blackboards.
    #[func]
    pub fn get_node_in_group(&self, group: GString) -> Variant {
        for (uid, obj) in &self.objects {
            if obj.bind().is_in_group(group.clone()) {
                return self
                    .source_objects
                    .get(uid)
                    .map(|n| n.clone().to_variant())
                    .unwrap_or(Variant::nil());
            }
        }
        Variant::nil()
    }

    /// Returns all live source nodes belonging to [param group].
    /// Use during action execution. Returns an empty array on simulation-cloned blackboards.
    #[func]
    pub fn get_nodes_in_group(&self, group: GString) -> Array<Variant> {
        let mut arr = Array::new();
        for (uid, obj) in &self.objects {
            if obj.bind().is_in_group(group.clone())
                && let Some(node) = self.source_objects.get(uid)
            {
                arr.push(&node.clone().to_variant());
            }
        }
        arr
    }

    /// Returns the [SimObjectProxy] whose UID matches the instance ID of [param node], or [code]null[/code] if not found.
    /// [param node_or_id] can be a [Node] instance, an [int] instance ID, or a [String] UID.
    #[func]
    pub fn get_object_for(&self, node_or_id: Variant) -> Variant {
        let uid = if let Ok(node) = node_or_id.clone().try_to::<Gd<Node>>() {
            node.instance_id().to_i64().to_string()
        } else if let Ok(id) = node_or_id.clone().try_to::<i64>() {
            id.to_string()
        } else if let Ok(uid_str) = node_or_id.clone().try_to::<GString>() {
            uid_str.to_string()
        } else {
            return Variant::nil();
        };

        if let Some(obj) = self.objects.get(&uid) {
            obj.clone().to_variant()
        } else {
            Variant::nil()
        }
    }

    /// Returns a copy whose properties and proxies can be changed without affecting this one.
    /// Property values are copied shallowly; live nodes are not carried over.
    #[func]
    pub fn clone_for_simulation(&self) -> Gd<GdPAIBlackboard> {
        let mut new_bb = GdPAIBlackboard::new_gd();
        {
            let mut new_bb_bind = new_bb.bind_mut();
            new_bb_bind.properties = self.properties.clone();

            for (uid, obj) in &self.objects {
                let mut new_obj = SimObjectProxy::new_gd();
                {
                    let mut new_obj_bind = new_obj.bind_mut();
                    let old_obj_bind = obj.bind();
                    new_obj_bind.uid = old_obj_bind.uid.clone();
                    new_obj_bind.groups = old_obj_bind.groups.clone();
                    new_obj_bind.properties = old_obj_bind.properties.clone();
                }
                new_bb_bind.objects.insert(uid.clone(), new_obj);
            }
        }
        new_bb
    }
}
