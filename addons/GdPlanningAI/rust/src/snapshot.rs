//! Send-safe, plain-data mirrors of Godot state ([`VariantSnapshot`], [`SimObjectData`],
//! [`BlackboardSnapshot`]) so planning can run off the main thread.
//!
//! Converting to or from live blackboards creates Godot objects, so it is main-thread only.
//! Snapshots are cloned per branch, so they stay cheap value types with `Eq`/`Hash`.

use crate::gdpai_blackboard::GdPAIBlackboard;
use crate::sim_object_proxy::SimObjectProxy;
use godot::prelude::*;
use std::collections::HashMap;

/// Send-safe mirror of a Godot [`Variant`]: primitives as Rust values, live objects as
/// instance-id handles, and everything else as opaque `var_to_bytes` blobs the planner can
/// compare but not inspect.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VariantSnapshot {
    /// A nil variant (also the fallback when a value cannot be serialised).
    Nil,
    /// Tier 1: a boolean.
    Bool(bool),
    /// Tier 1: an integer.
    Int(i64),
    /// Tier 1: a float, stored as its IEEE-754 bits (`f64::to_bits`) so the enum can be
    /// `Eq` and `Hash`. Convert with `f64::from_bits`.
    Float(u64),
    /// Tier 1: a string.
    Str(String),
    /// Tier 2: binary blob produced by `var_to_bytes`.
    Bytes(Vec<u8>),
    /// Tier 3: Godot instance-ID of a live object (opaque handle).
    ObjectRef(i64),
    /// Untyped array whose elements are snapshotted individually (so object handles and
    /// primitives inside it keep their tier).
    Array(Vec<VariantSnapshot>),
}

impl VariantSnapshot {
    /// Snapshots a [`Variant`]. Main thread only. Objects are caught before `var_to_bytes`, which
    /// would encode only a dangling id.
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
            return Self::Float(f.to_bits());
        }
        if let Ok(s) = v.try_to::<String>() {
            return Self::Str(s);
        }

        // Handle arrays - recursively snapshot each element
        if let Ok(array) = v.try_to::<Array<Variant>>() {
            let elements: Vec<VariantSnapshot> = array
                .iter_shared()
                .map(|elem| Self::from_variant(&elem))
                .collect();
            return Self::Array(elements);
        }

        // Tier 3: live Object — store instance ID BEFORE attempting var_to_bytes
        // (var_to_bytes succeeds on Objects but encodes them as EncodedObjectAsID)
        if let Ok(obj) = v.try_to::<Gd<Object>>() {
            return Self::ObjectRef(obj.instance_id().to_i64());
        }

        // Tier 2: Godot binary serialiser (for Vector2/3, Color, Dictionary, etc.)
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

    /// Rebuilds the [`Variant`]. Main thread only; a freed object becomes nil.
    pub fn to_variant(&self) -> Variant {
        match self {
            Self::Nil => Variant::nil(),
            Self::Bool(b) => b.to_variant(),
            Self::Int(i) => i.to_variant(),
            Self::Float(bits) => f64::from_bits(*bits).to_variant(),
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
            Self::Array(elements) => {
                let mut array = Array::<Variant>::new();
                for elem in elements {
                    array.push(&elem.to_variant());
                }
                array.to_variant()
            }
        }
    }

    /// Returns true if this variant is null/nil.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Nil)
    }

    /// Returns true if this variant is an empty string.
    pub fn is_empty_string(&self) -> bool {
        matches!(self, Self::Str(s) if s.is_empty())
    }
}

/// Send-safe mirror of [`SimObjectProxy`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimObjectData {
    /// The source node's instance id as a string; also this object's key in
    /// [`BlackboardSnapshot::objects`].
    pub uid: String,
    /// Group names the source node belongs to.
    pub groups: Vec<String>,
    /// The simulation-relevant properties the object exposed at snapshot time.
    pub properties: HashMap<String, VariantSnapshot>,
}

/// Send-safe mirror of [`GdPAIBlackboard`].
///
/// The planner thread operates exclusively on these. The
/// `GDPAI_OBJECTS` key is excluded from `properties`; world objects live
/// in `objects` as [`SimObjectData`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlackboardSnapshot {
    /// Plain properties of the blackboard.
    pub properties: HashMap<String, VariantSnapshot>,
    /// World objects keyed by uid (instance id as a string).
    pub objects: HashMap<String, SimObjectData>,
}

impl BlackboardSnapshot {
    /// Returns an object in `group`. Map order is unspecified, so use it for single-member groups.
    pub fn get_object_by_group(&self, group: &str) -> Option<&SimObjectData> {
        self.objects
            .values()
            .find(|obj| obj.groups.contains(&group.to_string()))
    }

    /// Returns the object with the given Godot instance ID, if any. Object-valued
    /// properties (`ObjectRef`) are resolved to world objects through this.
    pub fn get_object_by_instance_id(&self, id: i64) -> Option<&SimObjectData> {
        self.objects.get(&id.to_string())
    }

    /// Snapshots a live blackboard. Main thread only; `GDPAI_OBJECTS` is carried by `objects`.
    pub fn from_blackboard(bb: &GdPAIBlackboard) -> Self {
        let properties = bb
            .properties
            .iter()
            .filter(|(k, _)| k.as_str() != "GDPAI_OBJECTS")
            .map(|(k, v)| {
                let snap = VariantSnapshot::from_variant(v);
                (k.clone(), snap)
            })
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

    /// Rebuilds a simulation blackboard (proxies, no live nodes). Main thread only.
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
        properties.insert(
            "stamina".to_string(),
            VariantSnapshot::Float(75.5f64.to_bits()),
        );
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
        let snap = VariantSnapshot::Float(3.14159f64.to_bits());
        match snap {
            VariantSnapshot::Float(v) => {
                let f = f64::from_bits(v);
                assert!((f - 3.14159).abs() < f64::EPSILON)
            }
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
            VariantSnapshot::Float(v) => {
                let f = f64::from_bits(*v);
                assert!((f - 75.5).abs() < f64::EPSILON)
            }
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
