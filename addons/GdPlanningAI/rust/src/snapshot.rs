//! Send-safe snapshot types for planning.
//!
//! [`VariantSnapshot`], [`SimObjectData`], and [`BlackboardSnapshot`] mirror
//! their Godot-bound counterparts but contain only plain Rust data so they
//! can be sent across threads.

use crate::gdpai_blackboard::GdPAIBlackboard;
use crate::sim_object_proxy::SimObjectProxy;
use godot::prelude::*;
use std::collections::HashMap;

/// Send-safe mirror of a Godot [`Variant`].
///
/// Three tiers:
/// - **Tier 1** — primitives stored as plain Rust values (fast, comparable).
/// - **Tier 2** — everything else Godot can serialise via `var_to_bytes`.
/// - **Tier 3** — live `Object` references stored as instance-ID handles.
#[derive(Clone, Debug, PartialEq)]
pub enum VariantSnapshot {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// Tier 2: binary blob produced by `var_to_bytes`.
    Bytes(Vec<u8>),
    /// Tier 3: Godot instance-ID of a live object (opaque handle).
    ObjectRef(i64),
}

impl VariantSnapshot {
    /// Snapshot a [`Variant`]. **Must be called on the main thread.**
    pub fn from_variant(v: &Variant) -> Self {
        if v.is_nil() {
            return Self::Nil;
        }
        if let Ok(b) = v.try_to::<bool>() {
            return Self::Bool(b);
        }
        if let Ok(i) = v.try_to::<i64>() {
            return Self::Int(i);
        }
        if let Ok(f) = v.try_to::<f64>() {
            return Self::Float(f);
        }
        if let Ok(s) = v.try_to::<String>() {
            return Self::Str(s);
        }

        // Tier 3: live Object — store instance ID BEFORE attempting var_to_bytes
        // (var_to_bytes succeeds on Objects but encodes them as EncodedObjectAsID)
        if let Ok(obj) = v.try_to::<Gd<Object>>() {
            return Self::ObjectRef(obj.instance_id().to_i64());
        }

        // Tier 2: Godot binary serialiser (for Vector2/3, Color, Array, Dictionary, Resources, etc.)
        let bytes: PackedByteArray = godot::global::var_to_bytes(&v.clone());
        if !bytes.is_empty() {
            return Self::Bytes(bytes.to_vec());
        }

        log_warn!(
            "VariantSnapshot: could not snapshot value of type {:?}; storing as Nil.",
            v.get_type()
        );
        Self::Nil
    }

    /// Reconstruct a [`Variant`]. **Must be called on the main thread.**
    pub fn to_variant(&self) -> Variant {
        match self {
            Self::Nil => Variant::nil(),
            Self::Bool(b) => b.to_variant(),
            Self::Int(i) => i.to_variant(),
            Self::Float(f) => f.to_variant(),
            Self::Str(s) => s.to_variant(),
            Self::Bytes(b) => {
                let packed = PackedByteArray::from(b.as_slice());
                godot::global::bytes_to_var(&packed)
            }
            Self::ObjectRef(id) => {
                let instance_id = InstanceId::from_i64(*id);
                match Gd::<Object>::try_from_instance_id(instance_id) {
                    Ok(obj) => obj.to_variant(),
                    Err(_) => {
                        log_warn!(
                            "ObjectRef({}): object was freed before callback; returning Nil.",
                            id
                        );
                        Variant::nil()
                    }
                }
            }
        }
    }
}

/// Send-safe mirror of [`SimObjectProxy`].
#[derive(Clone, Debug)]
pub struct SimObjectData {
    pub uid: String,
    pub groups: Vec<String>,
    pub properties: HashMap<String, VariantSnapshot>,
}

/// Send-safe mirror of [`GdPAIBlackboard`].
///
/// The planner thread operates exclusively on these. The
/// `GDPAI_OBJECTS` key is excluded from `properties`; world objects live
/// in `objects` as [`SimObjectData`].
#[derive(Clone, Debug)]
pub struct BlackboardSnapshot {
    pub properties: HashMap<String, VariantSnapshot>,
    pub objects: HashMap<String, SimObjectData>,
}

impl BlackboardSnapshot {
    /// Snapshot a live [`GdPAIBlackboard`]. **Must be called on the main thread.**
    pub fn from_blackboard(bb: &GdPAIBlackboard) -> Self {
        let properties = bb
            .properties
            .iter()
            .filter(|(k, _)| k.as_str() != "GDPAI_OBJECTS")
            .map(|(k, v)| (k.clone(), VariantSnapshot::from_variant(v)))
            .collect();

        let objects = bb
            .objects
            .iter()
            .map(|(uid, proxy)| {
                let p = proxy.bind();
                let obj = SimObjectData {
                    uid: p.uid.clone(),
                    groups: p.groups.clone(),
                    properties: p
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), VariantSnapshot::from_variant(v)))
                        .collect(),
                };
                (uid.clone(), obj)
            })
            .collect();

        Self {
            properties,
            objects,
        }
    }

    /// Reconstruct a [`GdPAIBlackboard`] from this snapshot. **Main thread only.**
    pub fn into_blackboard(self) -> Gd<GdPAIBlackboard> {
        let mut bb = GdPAIBlackboard::new_gd();
        {
            let mut b = bb.bind_mut();
            b.properties = self
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.to_variant()))
                .collect();

            for (uid, obj_data) in self.objects {
                let mut proxy = SimObjectProxy::new_gd();
                {
                    let mut p = proxy.bind_mut();
                    p.uid = obj_data.uid;
                    p.groups = obj_data.groups;
                    p.properties = obj_data
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_variant()))
                        .collect();
                }
                b.objects.insert(uid, proxy);
            }
        }
        bb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_snapshot() -> BlackboardSnapshot {
        let mut properties = HashMap::new();
        properties.insert("health".to_string(), VariantSnapshot::Int(100));
        properties.insert("stamina".to_string(), VariantSnapshot::Float(75.5));
        properties.insert(
            "name".to_string(),
            VariantSnapshot::Str("TestAgent".to_string()),
        );
        properties.insert("is_alive".to_string(), VariantSnapshot::Bool(true));

        BlackboardSnapshot {
            properties,
            objects: HashMap::new(),
        }
    }

    #[test]
    fn variant_snapshot_nil_preserves_type() {
        let snap = VariantSnapshot::Nil;
        assert!(matches!(snap, VariantSnapshot::Nil));
    }

    #[test]
    fn variant_snapshot_int_preserves_value() {
        let snap = VariantSnapshot::Int(42);
        match snap {
            VariantSnapshot::Int(v) => assert_eq!(v, 42),
            _ => panic!("Expected Int variant"),
        }
    }

    #[test]
    fn variant_snapshot_float_preserves_value() {
        let snap = VariantSnapshot::Float(3.14159);
        match snap {
            VariantSnapshot::Float(v) => assert!((v - 3.14159).abs() < f64::EPSILON),
            _ => panic!("Expected Float variant"),
        }
    }

    #[test]
    fn blackboard_snapshot_retrieves_stored_values() {
        let snapshot = make_test_snapshot();

        // Test integer retrieval
        match snapshot.properties.get("health").unwrap() {
            VariantSnapshot::Int(v) => assert_eq!(*v, 100),
            _ => panic!("Expected Int"),
        }

        // Test float retrieval
        match snapshot.properties.get("stamina").unwrap() {
            VariantSnapshot::Float(v) => assert!((*v - 75.5).abs() < f64::EPSILON),
            _ => panic!("Expected Float"),
        }

        // Test string retrieval
        match snapshot.properties.get("name").unwrap() {
            VariantSnapshot::Str(s) => assert_eq!(s, "TestAgent"),
            _ => panic!("Expected Str"),
        }

        // Test bool retrieval
        match snapshot.properties.get("is_alive").unwrap() {
            VariantSnapshot::Bool(b) => assert!(*b),
            _ => panic!("Expected Bool"),
        }
    }

    #[test]
    fn blackboard_snapshot_missing_property_returns_none() {
        let snapshot = make_test_snapshot();
        assert!(snapshot.properties.get("nonexistent").is_none());
    }

    #[test]
    fn blackboard_snapshot_clone_creates_independent_copy() {
        let original = make_test_snapshot();
        let mut cloned = original.clone();

        // Modify clone
        cloned
            .properties
            .insert("new_prop".to_string(), VariantSnapshot::Int(42));

        // Original should be unchanged
        assert!(original.properties.get("new_prop").is_none());
        assert!(cloned.properties.get("new_prop").is_some());
    }
}
