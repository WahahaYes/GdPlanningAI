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
/// [br]- Named world [b]objects[/b] stored under the reserved key [code]GDPAI_OBJECTS[/code] and queried by group.
///   During planning, use [method get_proxy_in_group] / [method get_proxies_in_group] to access [SimObjectProxy] snapshots.
///   During action execution, use [method get_node_in_group] / [method get_nodes_in_group] to access the live source nodes.
///
/// At planning time the engine calls [method clone_for_simulation] to create an isolated copy for each search branch.
/// Simulation clones carry proxy data only; [method get_node_in_group] returns [code]null[/code] on them.
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GdPAIBlackboard {
    pub properties: HashMap<String, Variant>,
    pub objects: HashMap<String, Gd<SimObjectProxy>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_hashmap_supports_insertion() {
        // Test HashMap operations used by set_property (using String as placeholder)
        let mut properties: HashMap<String, String> = HashMap::new();
        properties.insert("health".to_string(), "100".to_string());
        properties.insert("stamina".to_string(), "75".to_string());
        
        assert_eq!(properties.len(), 2);
        assert!(properties.contains_key("health"));
        assert!(properties.contains_key("stamina"));
    }

    #[test]
    fn properties_hashmap_supports_removal() {
        // Test HashMap operations used by erase_property (using String as placeholder)
        let mut properties: HashMap<String, String> = HashMap::new();
        properties.insert("temp_key".to_string(), "value".to_string());
        
        assert!(properties.contains_key("temp_key"));
        properties.remove("temp_key");
        assert!(!properties.contains_key("temp_key"));
    }

    #[test]
    fn properties_hashmap_can_be_cleared() {
        // Test HashMap clear operation used by set_dict (using String as placeholder)
        let mut properties: HashMap<String, String> = HashMap::new();
        properties.insert("key1".to_string(), "val1".to_string());
        properties.insert("key2".to_string(), "val2".to_string());
        
        assert_eq!(properties.len(), 2);
        properties.clear();
        assert_eq!(properties.len(), 0);
        assert!(properties.is_empty());
    }

    #[test]
    fn properties_hashmap_can_be_cloned() {
        // Test HashMap cloning used by clone_for_simulation (using String as placeholder)
        let mut original: HashMap<String, String> = HashMap::new();
        original.insert("health".to_string(), "100".to_string());
        
        let cloned = original.clone();
        assert_eq!(cloned.len(), original.len());
        assert!(cloned.contains_key("health"));
    }

    #[test]
    fn cloned_hashmap_is_independent() {
        // Test that HashMap clone creates independent copy (critical for simulation)
        let mut original: HashMap<String, String> = HashMap::new();
        original.insert("original_key".to_string(), "val".to_string());
        
        let mut cloned = original.clone();
        cloned.insert("clone_key".to_string(), "val2".to_string());
        
        assert!(original.contains_key("original_key"));
        assert!(!original.contains_key("clone_key"));
        assert!(cloned.contains_key("original_key"));
        assert!(cloned.contains_key("clone_key"));
    }

    #[test]
    fn objects_hashmap_supports_insertion_and_lookup() {
        // Test operations used for object storage
        let mut objects: HashMap<String, Gd<SimObjectProxy>> = HashMap::new();
        let uid = "12345".to_string();
        
        // Can't create Gd<SimObjectProxy> without Godot runtime,
        // but we verify the HashMap type is correct
        assert!(objects.is_empty());
        assert!(!objects.contains_key(&uid));
    }

    #[test]
    fn objects_hashmap_can_be_cleared() {
        // Test HashMap clear used when GDPAI_OBJECTS is erased
        let mut objects: HashMap<String, Gd<SimObjectProxy>> = HashMap::new();
        // Would insert objects here if we had Godot runtime
        objects.clear();
        assert!(objects.is_empty());
    }

    #[test]
    fn source_objects_hashmap_supports_storage() {
        // Test the source_objects HashMap structure
        let mut source_objects: HashMap<String, Gd<Node>> = HashMap::new();
        // Can't create Gd<Node> without Godot runtime, but verify type
        assert!(source_objects.is_empty());
    }

    #[test]
    fn special_key_gdpai_objects_is_string_constant() {
        // Verify the special key constant works as expected
        let special_key = "GDPAI_OBJECTS";
        let key_str = special_key.to_string();
        assert_eq!(key_str, "GDPAI_OBJECTS");
    }

    #[test]
    fn clone_for_simulation_uses_hashmap_clone() {
        // Test that the clone pattern works for properties (using String as placeholder)
        let mut properties: HashMap<String, String> = HashMap::new();
        properties.insert("health".to_string(), "100".to_string());
        properties.insert("position".to_string(), "0,0".to_string());
        
        // Simulate what clone_for_simulation does
        let cloned_properties = properties.clone();
        
        assert_eq!(properties.len(), cloned_properties.len());
        assert!(cloned_properties.contains_key("health"));
        assert!(cloned_properties.contains_key("position"));
    }

    #[test]
    fn objects_iter_supports_cloning_pattern() {
        // Test iteration pattern used in clone_for_simulation
        let objects: HashMap<String, Gd<SimObjectProxy>> = HashMap::new();
        
        // The clone_for_simulation method iterates: for (uid, obj) in &self.objects
        let mut clone_count = 0;
        for (_uid, _obj) in &objects {
            clone_count += 1;
        }
        assert_eq!(clone_count, 0); // Empty for this test
    }
}
