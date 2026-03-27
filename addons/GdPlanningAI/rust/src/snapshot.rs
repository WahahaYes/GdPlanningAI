//! Send-safe snapshot types for background planning.
//!
//! [`VariantSnapshot`], [`SimObjectData`], and [`BlackboardSnapshot`] mirror
//! their Godot-bound counterparts but contain only plain Rust data so they
//! can be sent across threads.

use crate::gdpai_blackboard::GdPAIBlackboard;
use crate::sim_object_proxy::SimObjectProxy;
use godot::prelude::*;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// VariantSnapshot
// ---------------------------------------------------------------------------

/// Send-safe mirror of a Godot [`Variant`].
///
/// Three tiers:
/// - **Tier 1** — primitives stored as plain Rust values (fast, comparable).
/// - **Tier 2** — everything else Godot can serialise via `var_to_bytes`.
/// - **Tier 3** — live `Object` references stored as instance-ID handles.
#[derive(Clone, Debug)]
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

        // Tier 2: Godot binary serialiser
        let bytes: PackedByteArray = godot::global::var_to_bytes(&v.clone());
        if !bytes.is_empty() {
            return Self::Bytes(bytes.to_vec());
        }

        // Tier 3: live Object — store instance ID
        if let Ok(obj) = v.try_to::<Gd<Object>>() {
            return Self::ObjectRef(obj.instance_id().to_i64());
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

// ---------------------------------------------------------------------------
// SimObjectData
// ---------------------------------------------------------------------------

/// Send-safe mirror of [`SimObjectProxy`].
#[derive(Clone, Debug)]
pub struct SimObjectData {
    pub uid: String,
    pub groups: Vec<String>,
    pub properties: HashMap<String, VariantSnapshot>,
}

// ---------------------------------------------------------------------------
// BlackboardSnapshot
// ---------------------------------------------------------------------------

/// Send-safe mirror of [`GdPAIBlackboard`].
///
/// The background planning thread operates exclusively on these. The
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
