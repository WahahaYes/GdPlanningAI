//! Blackboard that holds agent and world state during planning.
//!
//! A [`GdPAIBlackboard`] stores named scalar properties and a map of
//! [`SimObjectProxy`] objects. The engine clones it into isolated
//! simulation branches via [`GdPAIBlackboard::clone_for_simulation`].

use crate::sim_object_proxy::SimObjectProxy;
use godot::prelude::*;
use std::collections::HashMap;

/// Key/value store for agent or world state, accessible from GDScript.
///
/// A blackboard holds two kinds of data:
/// [br]- Named scalar [b]properties[/b] (booleans, numbers, strings, etc.) accessed via [method get_property] and [method set_property].
/// [br]- Named world [b]objects[/b] ([SimObjectProxy] snapshots) stored under the reserved key [code]GDPAI_OBJECTS[/code] and queried by group.
///
/// At planning time the engine calls [method clone_for_simulation] to create an isolated copy for each search branch.
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GdPAIBlackboard {
    pub properties: HashMap<String, Variant>,
    pub objects: HashMap<String, Gd<SimObjectProxy>>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for GdPAIBlackboard {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            properties: HashMap::new(),
            objects: HashMap::new(),
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
            if let Ok(objects_array) = value.clone().try_to::<Array<Variant>>() {
                for obj_var in objects_array.iter_shared() {
                    match obj_var.try_to::<Gd<Object>>() {
                        Ok(obj_gd) => {
                            if let Some(sim_obj) =
                                crate::sim_object_proxy::SimObjectProxy::from_object_data(obj_gd)
                            {
                                let uid = sim_obj.bind().uid.clone();
                                self.objects.insert(uid, sim_obj);
                            }
                        }
                        Err(_) => {
                            log_debug!(
                                "GDPAI_OBJECTS: skipping item that is not a RefCounted object"
                            );
                        }
                    }
                }
            }
            log_debug!("GDPAI_OBJECTS: registered {} object(s)", self.objects.len());
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
        for (k, v) in dict.iter_shared() {
            if let Ok(key) = k.try_to::<GString>() {
                self.set_property(key, v);
            }
        }
    }

    /// Returns all [SimObjectProxy] objects that belong to [param group].
    #[func]
    pub fn get_objects_in_group(&self, group: GString) -> Array<Gd<SimObjectProxy>> {
        let mut arr = Array::new();
        for obj in self.objects.values() {
            if obj.bind().is_in_group(group.clone()) {
                arr.push(obj);
            }
        }
        arr
    }

    /// Returns the first [SimObjectProxy] belonging to [param group], or [code]null[/code] if none exists.
    #[func]
    pub fn get_first_object_in_group(&self, group: GString) -> Variant {
        for obj in self.objects.values() {
            if obj.bind().is_in_group(group.clone()) {
                return obj.clone().to_variant();
            }
        }
        Variant::nil()
    }

    /// Returns the [SimObjectProxy] whose UID matches the instance ID of [param node], or [code]null[/code] if not found.
    #[func]
    pub fn get_object_for(&self, node: Gd<Node>) -> Variant {
        let uid = node.instance_id().to_i64().to_string();
        if let Some(obj) = self.objects.get(&uid) {
            obj.clone().to_variant()
        } else {
            Variant::nil()
        }
    }

    /// Creates a deep clone of this blackboard for use in a planning simulation branch.
    ///
    /// All properties and [`SimObjectProxy`] objects are copied. Mutations to the
    /// clone during simulation do not affect the original.
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
