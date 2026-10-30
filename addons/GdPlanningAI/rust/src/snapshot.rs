//! Send-safe, plain-data mirrors of Godot state ([`VariantSnapshot`], [`SimObjectData`],
//! [`BlackboardSnapshot`]) so planning can run off the main thread.
//!
//! Converting to or from live blackboards creates Godot objects, so it is main-thread only.
//! Branches share snapshots behind `Arc`; an effect produces a new one. They stay plain value
//! types with `Eq`/`Hash`.

use crate::gdpai_blackboard::GdPAIBlackboard;
use crate::sim_object_proxy::SimObjectProxy;
use godot::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Send-safe mirror of a Godot [`Variant`]: primitives as Rust values, live objects as
/// instance-id handles, and everything else as opaque `var_to_bytes` blobs the planner can
/// compare but not inspect.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "SimObjectDataShadow", into = "SimObjectDataShadow")]
pub struct SimObjectData {
    /// The source node's instance id as a string; also this object's key in
    /// [`BlackboardSnapshot::objects`].
    pub uid: String,
    /// Group names the source node belongs to.
    pub groups: Vec<String>,
    /// The simulation-relevant properties the object exposed at snapshot time.
    /// Shared structurally: untouched objects keep the same [`Arc`] across patches.
    pub properties: Arc<HashMap<String, VariantSnapshot>>,
}

/// Serde shadow of [`SimObjectData`]: serde implements `Serialize` but not `Deserialize`
/// for `Arc`, so snapshots cross the JSON boundary (probe capture/replay) unshared and are
/// re-wrapped on load.
#[derive(Serialize, Deserialize)]
struct SimObjectDataShadow {
    uid: String,
    groups: Vec<String>,
    properties: HashMap<String, VariantSnapshot>,
}

impl From<SimObjectDataShadow> for SimObjectData {
    fn from(shadow: SimObjectDataShadow) -> Self {
        Self {
            uid: shadow.uid,
            groups: shadow.groups,
            properties: Arc::new(shadow.properties),
        }
    }
}

impl From<SimObjectData> for SimObjectDataShadow {
    fn from(data: SimObjectData) -> Self {
        Self {
            uid: data.uid,
            groups: data.groups,
            properties: (*data.properties).clone(),
        }
    }
}

/// Send-safe mirror of [`GdPAIBlackboard`].
///
/// The planner thread operates exclusively on these. The
/// `GDPAI_OBJECTS` key is excluded from `properties`; world objects live
/// in `objects` as [`SimObjectData`]. Both maps are structurally shared behind
/// [`Arc`]: cloning a snapshot bumps refcounts, and patches copy-on-write only
/// the maps they touch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "BlackboardSnapshotShadow", into = "BlackboardSnapshotShadow")]
pub struct BlackboardSnapshot {
    /// Plain properties of the blackboard.
    pub properties: Arc<HashMap<String, VariantSnapshot>>,
    /// World objects keyed by uid (instance id as a string).
    pub objects: Arc<HashMap<String, Arc<SimObjectData>>>,
}

/// Serde shadow of [`BlackboardSnapshot`]: see [`SimObjectDataShadow`].
#[derive(Serialize, Deserialize)]
struct BlackboardSnapshotShadow {
    properties: HashMap<String, VariantSnapshot>,
    objects: HashMap<String, SimObjectData>,
}

impl From<BlackboardSnapshotShadow> for BlackboardSnapshot {
    fn from(shadow: BlackboardSnapshotShadow) -> Self {
        Self {
            properties: Arc::new(shadow.properties),
            objects: Arc::new(
                shadow
                    .objects
                    .into_iter()
                    .map(|(uid, obj)| (uid, Arc::new(obj)))
                    .collect(),
            ),
        }
    }
}

impl From<BlackboardSnapshot> for BlackboardSnapshotShadow {
    fn from(snap: BlackboardSnapshot) -> Self {
        Self {
            properties: (*snap.properties).clone(),
            objects: snap
                .objects
                .iter()
                .map(|(uid, obj)| (uid.clone(), (**obj).clone()))
                .collect(),
        }
    }
}

impl BlackboardSnapshot {
    /// Returns an object in `group`. Map order is unspecified, so use it for single-member groups.
    pub fn get_object_by_group(&self, group: &str) -> Option<&SimObjectData> {
        self.objects
            .values()
            .find(|obj| obj.groups.contains(&group.to_string()))
            .map(|arc| arc.as_ref())
    }

    /// Returns the object with the given Godot instance ID, if any. Object-valued
    /// properties (`ObjectRef`) are resolved to world objects through this.
    pub fn get_object_by_instance_id(&self, id: i64) -> Option<&SimObjectData> {
        self.objects.get(&id.to_string()).map(|arc| arc.as_ref())
    }

    /// Snapshots a live blackboard. Main thread only; `GDPAI_OBJECTS` is carried by `objects`.
    pub fn from_blackboard(bb: &GdPAIBlackboard) -> Self {
        let properties: HashMap<String, VariantSnapshot> = bb
            .properties
            .iter()
            .filter(|(k, _)| k.as_str() != "GDPAI_OBJECTS")
            .map(|(k, v)| {
                let snap = VariantSnapshot::from_variant(v);
                (k.clone(), snap)
            })
            .collect();

        let objects: HashMap<String, Arc<SimObjectData>> = bb
            .objects
            .iter()
            .map(|(uid, proxy)| {
                let p = proxy.bind();
                let obj = SimObjectData {
                    uid: p.uid.clone(),
                    groups: p.groups.clone(),
                    properties: Arc::new(
                        p.properties
                            .iter()
                            .map(|(k, v)| (k.clone(), VariantSnapshot::from_variant(v)))
                            .collect(),
                    ),
                };
                (uid.clone(), Arc::new(obj))
            })
            .collect();

        Self {
            properties: Arc::new(properties),
            objects: Arc::new(objects),
        }
    }

    /// Rebuilds a simulation blackboard (proxies, no live nodes). Main thread only.
    ///
    /// Borrows the snapshot so [`Self::patch_from`] can diff the board back onto the base
    /// it was built from. Fresh boards start with empty dirty sets: they match the
    /// snapshot exactly, and every later mutation goes through a marking mutator.
    pub fn into_blackboard(&self) -> Gd<GdPAIBlackboard> {
        let mut bb = GdPAIBlackboard::new_gd();
        {
            let mut b = bb.bind_mut();
            b.properties = self
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.to_variant()))
                .collect();

            for (uid, obj_data) in self.objects.iter() {
                let mut proxy = SimObjectProxy::new_gd();
                {
                    let mut p = proxy.bind_mut();
                    p.uid = obj_data.uid.clone();
                    p.groups = obj_data.groups.clone();
                    p.properties = obj_data
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_variant()))
                        .collect();
                }
                b.objects.insert(uid.clone(), proxy);
            }
        }
        bb
    }

    /// Diffs a mutated simulation board back onto its base snapshot. Main thread only.
    ///
    /// `old` is the snapshot the board was built from ([`Self::into_blackboard`]); `bb`
    /// is that board after the effect callable ran. Clones the base maps, then re-encodes
    /// only dirty keys plus erasures and the aliasing cover, so untouched entries pay no
    /// `from_variant` cost. Must equal [`Self::from_blackboard`] on the same board.
    pub fn patch_from(old: &BlackboardSnapshot, bb: &GdPAIBlackboard) -> Self {
        let guards: HashMap<String, _> = bb
            .objects
            .iter()
            .map(|(uid, proxy)| (uid.clone(), proxy.bind()))
            .collect();
        let live_objects: HashMap<String, LiveObject<'_, Variant>> = guards
            .iter()
            .map(|(uid, guard)| {
                (
                    uid.clone(),
                    LiveObject {
                        uid: &guard.uid,
                        groups: &guard.groups,
                        properties: &guard.properties,
                        dirty: &guard.dirty,
                    },
                )
            })
            .collect();
        Self::patch_with(
            old,
            &bb.properties,
            &bb.dirty_props,
            &live_objects,
            &bb.dirty_objects,
            VariantSnapshot::from_variant,
        )
    }

    /// Engine-free diff core behind [`Self::patch_from`]: `encode` stands in for
    /// [`VariantSnapshot::from_variant`], so unit tests can pin the merge logic and count
    /// encodings without a Godot engine.
    fn patch_with<V>(
        base: &BlackboardSnapshot,
        live_props: &HashMap<String, V>,
        props_dirty: &HashSet<String>,
        live_objects: &HashMap<String, LiveObject<'_, V>>,
        objects_dirty: &HashSet<String>,
        encode: impl Fn(&V) -> VariantSnapshot,
    ) -> BlackboardSnapshot {
        // Structural sharing: clone the Arcs (refcount bumps), then copy-on-write the
        // maps before patching. Untouched entries keep their inner Arcs, so untouched
        // objects stay `ptr_eq` with the base after the patch. A fully untouched patch
        // skips the copy entirely and shares the top-level maps too (see below).
        let properties = if props_dirty.is_empty()
            && aliased_keys(&base.properties).next().is_none()
            && !base.properties.contains_key("GDPAI_OBJECTS")
        {
            // Nothing dirty, no aliasing cover, and no reserved key to strip: the patched
            // map would equal the base one, so share its Arc instead of cloning.
            Arc::clone(&base.properties)
        } else {
            let mut properties = Arc::clone(&base.properties);
            {
                let props_mut = Arc::make_mut(&mut properties);
                patch_map(
                    &base.properties,
                    props_mut,
                    live_props,
                    props_dirty,
                    &encode,
                );
                // Mirror from_blackboard: GDPAI_OBJECTS is carried by objects, never properties.
                props_mut.remove("GDPAI_OBJECTS");
            }
            properties
        };

        let objects = if objects_untouched(base, live_objects, objects_dirty) {
            // No presence change, nothing flagged, no per-proxy dirt and no aliasing cover:
            // every entry would be skipped in place, so share the base map outright.
            Arc::clone(&base.objects)
        } else {
            let mut objects = Arc::clone(&base.objects);
            {
                let objects_mut = Arc::make_mut(&mut objects);
                // Presence first: base objects missing from the live board are erased, even when
                // nobody flagged them — live wins so patch matches a full re-snapshot.
                objects_mut.retain(|uid, _| live_objects.contains_key(uid));
                let no_base_props: HashMap<String, VariantSnapshot> = HashMap::new();
                for (uid, live) in live_objects {
                    if objects_dirty.contains(uid) || !live.dirty.is_empty() {
                        let arc_entry = objects_mut.entry(uid.clone()).or_insert_with(|| {
                            Arc::new(SimObjectData {
                                uid: live.uid.to_string(),
                                groups: live.groups.to_vec(),
                                properties: Arc::new(HashMap::new()),
                            })
                        });
                        let entry = Arc::make_mut(arc_entry);
                        entry.uid = live.uid.to_string();
                        entry.groups = live.groups.to_vec();
                        let base_props = base
                            .objects
                            .get(uid)
                            .map(|obj| &*obj.properties)
                            .unwrap_or(&no_base_props);
                        let props_mut = Arc::make_mut(&mut entry.properties);
                        patch_map(base_props, props_mut, live.properties, live.dirty, &encode);
                        // Rebuilt proxies only mark current keys dirty, so prune keys the live
                        // proxy dropped; otherwise stale Tier-1 entries survive from the base clone.
                        if objects_dirty.contains(uid) {
                            props_mut.retain(|key, _| live.properties.contains_key(key));
                        }
                    } else if let Some(arc_entry) = objects_mut.get_mut(uid) {
                        // Untouched proxy: only the aliasing cover can change anything. When no
                        // aliased keys exist the Arc stays shared (ptr_eq with base). Collect the
                        // keys first so the base scan and the mutation do not alias.
                        let aliased: Vec<String> =
                            aliased_keys(&arc_entry.properties).cloned().collect();
                        if aliased.is_empty() {
                            continue;
                        }
                        let entry = Arc::make_mut(arc_entry);
                        let props_mut = Arc::make_mut(&mut entry.properties);
                        for key in &aliased {
                            match live.properties.get(key) {
                                Some(v) => {
                                    props_mut.insert(key.clone(), encode(v));
                                }
                                None => {
                                    props_mut.remove(key);
                                }
                            }
                        }
                    } else {
                        // A live object the base never had and nobody flagged cannot arise through
                        // the marking mutators; snapshot it fully so live still wins.
                        objects_mut.insert(
                            uid.clone(),
                            Arc::new(SimObjectData {
                                uid: live.uid.to_string(),
                                groups: live.groups.to_vec(),
                                properties: Arc::new(
                                    live.properties
                                        .iter()
                                        .map(|(k, v)| (k.clone(), encode(v)))
                                        .collect(),
                                ),
                            }),
                        );
                    }
                }
            }
            objects
        };
        BlackboardSnapshot {
            properties,
            objects,
        }
    }
}

/// Engine-free view of one live world object for diff-back.
struct LiveObject<'a, V> {
    /// Proxy uid (the board's object-map key).
    uid: &'a str,
    /// Proxy groups; fixed for the proxy's lifetime.
    groups: &'a [String],
    /// Live simulation properties.
    properties: &'a HashMap<String, V>,
    /// Keys mutated since the proxy was created.
    dirty: &'a HashSet<String>,
}

/// Merges one property map: dirty keys are re-encoded (or erased when the live board
/// dropped them), and untouched [`VariantSnapshot::Bytes`]/[`VariantSnapshot::Array`]
/// base entries are re-encoded as aliasing cover — GDScript may hold a shared
/// Dictionary/Array and mutate it in place without any marking call. Tier 1 values and
/// object handles are owned copies, so untouched ones are reused as-is.
/// Note: a freed ObjectRef re-encodes as Nil but patch reuses the stale handle — effects must not free scene objects.
fn patch_map<V>(
    base: &HashMap<String, VariantSnapshot>,
    patched: &mut HashMap<String, VariantSnapshot>,
    live: &HashMap<String, V>,
    dirty: &HashSet<String>,
    encode: impl Fn(&V) -> VariantSnapshot,
) {
    for key in dirty {
        match live.get(key) {
            Some(v) => {
                patched.insert(key.clone(), encode(v));
            }
            None => {
                patched.remove(key);
            }
        }
    }
    // Dirty keys were just encoded above; re-encoding them here would double-count.
    for key in aliased_keys(base) {
        if dirty.contains(key) {
            continue;
        }
        match live.get(key) {
            Some(v) => {
                patched.insert(key.clone(), encode(v));
            }
            None => {
                patched.remove(key);
            }
        }
    }
}

/// True when no object entry can change: nothing flagged, the live key set matches the
/// base one, no proxy reports dirt, and no base object needs the aliasing cover. The
/// caller then shares the base objects map outright instead of cloning it.
fn objects_untouched<V>(
    base: &BlackboardSnapshot,
    live_objects: &HashMap<String, LiveObject<'_, V>>,
    objects_dirty: &HashSet<String>,
) -> bool {
    objects_dirty.is_empty()
        && live_objects.len() == base.objects.len()
        && live_objects.iter().all(|(uid, live)| {
            live.dirty.is_empty()
                && base
                    .objects
                    .get(uid)
                    .is_some_and(|obj| aliased_keys(&obj.properties).next().is_none())
        })
}

/// Keys whose base snapshot needs the aliasing cover: [`VariantSnapshot::Bytes`] and
/// [`VariantSnapshot::Array`] values may share storage with GDScript-side handles.
fn aliased_keys(base: &HashMap<String, VariantSnapshot>) -> impl Iterator<Item = &String> + '_ {
    base.iter().filter_map(|(key, snap)| {
        matches!(snap, VariantSnapshot::Bytes(_) | VariantSnapshot::Array(_)).then_some(key)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Engine-free stand-in for live values: Tier 1 ints, Tier 2 blobs, arrays.
    #[derive(Clone, Debug, PartialEq)]
    enum TestValue {
        Int(i64),
        Blob(Vec<u8>),
        List(Vec<TestValue>),
    }

    fn encode_value(v: &TestValue) -> VariantSnapshot {
        match v {
            TestValue::Int(i) => VariantSnapshot::Int(*i),
            TestValue::Blob(b) => VariantSnapshot::Bytes(b.clone()),
            TestValue::List(items) => {
                VariantSnapshot::Array(items.iter().map(encode_value).collect())
            }
        }
    }

    /// Counting encoder: stands in for `VariantSnapshot::from_variant`.
    fn counting<'a>(count: &'a Cell<usize>) -> impl Fn(&TestValue) -> VariantSnapshot + 'a {
        |v: &TestValue| {
            count.set(count.get() + 1);
            encode_value(v)
        }
    }

    fn props(pairs: &[(&str, TestValue)]) -> HashMap<String, TestValue> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn snaps(pairs: &[(&str, VariantSnapshot)]) -> HashMap<String, VariantSnapshot> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn base_snapshot(
        properties: HashMap<String, VariantSnapshot>,
        objects: HashMap<String, Arc<SimObjectData>>,
    ) -> BlackboardSnapshot {
        BlackboardSnapshot {
            properties: Arc::new(properties),
            objects: Arc::new(objects),
        }
    }

    fn live<'a>(
        uid: &'a str,
        properties: &'a HashMap<String, TestValue>,
        dirty: &'a HashSet<String>,
    ) -> LiveObject<'a, TestValue> {
        const GROUPS: &[String] = &[];
        LiveObject {
            uid,
            groups: GROUPS,
            properties,
            dirty,
        }
    }

    fn set(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| k.to_string()).collect()
    }

    fn patch(
        base: &BlackboardSnapshot,
        live_props: &HashMap<String, TestValue>,
        props_dirty: &HashSet<String>,
        live_objects: &HashMap<String, LiveObject<'_, TestValue>>,
        objects_dirty: &HashSet<String>,
        count: &Cell<usize>,
    ) -> BlackboardSnapshot {
        BlackboardSnapshot::patch_with(
            base,
            live_props,
            props_dirty,
            live_objects,
            objects_dirty,
            counting(count),
        )
    }

    #[test]
    fn set_erase_and_new_key_match_live() {
        let base = base_snapshot(
            snaps(&[
                ("a", VariantSnapshot::Int(1)),
                ("b", VariantSnapshot::Int(2)),
                ("gone", VariantSnapshot::Int(9)),
                ("cfg", VariantSnapshot::Bytes(vec![1])),
            ]),
            HashMap::new(),
        );
        let live = props(&[
            ("a", TestValue::Int(1)),
            ("b", TestValue::Int(3)),
            ("c", TestValue::Int(4)),
            ("cfg", TestValue::Blob(vec![1, 2])),
        ]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &live,
            &set(&["b", "c", "gone", "cfg"]),
            &HashMap::new(),
            &HashSet::new(),
            &count,
        );
        // `gone` erases without encoding; `cfg` encodes once despite also matching
        // the aliasing cover; untouched Tier 1 `a` is reused.
        assert_eq!(
            *patched.properties,
            snaps(&[
                ("a", VariantSnapshot::Int(1)),
                ("b", VariantSnapshot::Int(3)),
                ("c", VariantSnapshot::Int(4)),
                ("cfg", VariantSnapshot::Bytes(vec![1, 2])),
            ])
        );
        assert_eq!(count.get(), 3);
    }

    #[test]
    fn bulk_replace_marks_everything() {
        let base = base_snapshot(
            snaps(&[
                ("a", VariantSnapshot::Int(1)),
                ("keep", VariantSnapshot::Int(2)),
            ]),
            HashMap::new(),
        );
        let live = props(&[("keep", TestValue::Int(2)), ("new", TestValue::Int(3))]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &live,
            &set(&["a", "keep", "new"]),
            &HashMap::new(),
            &HashSet::new(),
            &count,
        );
        assert_eq!(
            *patched.properties,
            snaps(&[
                ("keep", VariantSnapshot::Int(2)),
                ("new", VariantSnapshot::Int(3)),
            ])
        );
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn proxy_write_encodes_only_its_dirty_key() {
        let mut base_objects: HashMap<String, Arc<SimObjectData>> = HashMap::new();
        base_objects.insert(
            "7".to_string(),
            Arc::new(SimObjectData {
                uid: "7".to_string(),
                groups: vec![],
                properties: Arc::new(snaps(&[
                    ("fuel", VariantSnapshot::Int(5)),
                    ("wood", VariantSnapshot::Int(1)),
                ])),
            }),
        );
        let base = base_snapshot(HashMap::new(), base_objects);
        let live_props = props(&[("fuel", TestValue::Int(3)), ("wood", TestValue::Int(1))]);
        let proxy_dirty = set(&["fuel"]);
        let live_objects = HashMap::from([("7".to_string(), live("7", &live_props, &proxy_dirty))]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &HashMap::new(),
            &HashSet::new(),
            &live_objects,
            &HashSet::new(),
            &count,
        );
        assert_eq!(
            *patched.objects["7"].properties,
            snaps(&[
                ("fuel", VariantSnapshot::Int(3)),
                ("wood", VariantSnapshot::Int(1)),
            ])
        );
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn object_rebuild_refreshes_and_prunes() {
        let mut base_objects: HashMap<String, Arc<SimObjectData>> = HashMap::new();
        for (uid, fuel) in [("7", 1), ("9", 1)] {
            base_objects.insert(
                uid.to_string(),
                Arc::new(SimObjectData {
                    uid: uid.to_string(),
                    groups: vec![],
                    properties: Arc::new(snaps(&[("fuel", VariantSnapshot::Int(fuel))])),
                }),
            );
        }
        let base = base_snapshot(HashMap::new(), base_objects);
        // uid 7 rebuilt (all keys dirty), uid 11 added, uid 9 dropped.
        let live7 = props(&[("fuel", TestValue::Int(2)), ("extra", TestValue::Int(3))]);
        let live11 = props(&[("fuel", TestValue::Int(9))]);
        let dirty7 = set(&["fuel", "extra"]);
        let dirty11 = set(&["fuel"]);
        let live_objects = HashMap::from([
            ("7".to_string(), live("7", &live7, &dirty7)),
            ("11".to_string(), live("11", &live11, &dirty11)),
        ]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &HashMap::new(),
            &HashSet::new(),
            &live_objects,
            &set(&["7", "9"]),
            &count,
        );
        assert!(!patched.objects.contains_key("9"));
        assert_eq!(
            *patched.objects["7"].properties,
            snaps(&[
                ("fuel", VariantSnapshot::Int(2)),
                ("extra", VariantSnapshot::Int(3)),
            ])
        );
        assert_eq!(
            *patched.objects["11"].properties,
            snaps(&[("fuel", VariantSnapshot::Int(9))])
        );
        assert_eq!(count.get(), 3);
    }

    #[test]
    fn gdpai_objects_key_never_enters_patched_properties() {
        let base = base_snapshot(
            snaps(&[("hunger", VariantSnapshot::Int(1))]),
            HashMap::new(),
        );
        // Live board has the reserved key set (as properties always does); from
        // would exclude it, so patch must too.
        let live = props(&[
            ("hunger", TestValue::Int(1)),
            ("GDPAI_OBJECTS", TestValue::Int(0)),
        ]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &live,
            &set(&["GDPAI_OBJECTS"]),
            &HashMap::new(),
            &HashSet::new(),
            &count,
        );
        assert!(!patched.properties.contains_key("GDPAI_OBJECTS"));
        assert_eq!(
            *patched.properties,
            snaps(&[("hunger", VariantSnapshot::Int(1))])
        );
    }

    #[test]
    fn rebuilt_proxy_shrinking_keys_prunes_dropped_tier1() {
        let mut base_objects: HashMap<String, Arc<SimObjectData>> = HashMap::new();
        base_objects.insert(
            "7".to_string(),
            Arc::new(SimObjectData {
                uid: "7".to_string(),
                groups: vec![],
                properties: Arc::new(snaps(&[
                    ("fuel", VariantSnapshot::Int(5)),
                    ("extra", VariantSnapshot::Int(9)),
                ])),
            }),
        );
        let base = base_snapshot(HashMap::new(), base_objects);
        // Rebuild shrank the proxy: mark_all_dirty marked only the surviving key.
        let live_props = props(&[("fuel", TestValue::Int(5))]);
        let proxy_dirty = set(&["fuel"]);
        let live_objects = HashMap::from([("7".to_string(), live("7", &live_props, &proxy_dirty))]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &HashMap::new(),
            &HashSet::new(),
            &live_objects,
            &set(&["7"]),
            &count,
        );
        assert_eq!(
            *patched.objects["7"].properties,
            snaps(&[("fuel", VariantSnapshot::Int(5))])
        );
        assert!(!patched.objects["7"].properties.contains_key("extra"));
    }

    #[test]
    fn unmarked_aliased_writes_are_picked_up() {
        let base = base_snapshot(
            snaps(&[
                ("cfg", VariantSnapshot::Bytes(vec![1, 2])),
                (
                    "tags",
                    VariantSnapshot::Array(vec![VariantSnapshot::Int(1)]),
                ),
                ("n", VariantSnapshot::Int(0)),
            ]),
            HashMap::new(),
        );
        // No marking call fired, but the shared handles changed in place.
        let live = props(&[
            ("cfg", TestValue::Blob(vec![1, 2, 3])),
            (
                "tags",
                TestValue::List(vec![TestValue::Int(1), TestValue::Int(2)]),
            ),
            ("n", TestValue::Int(0)),
        ]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &live,
            &HashSet::new(),
            &HashMap::new(),
            &HashSet::new(),
            &count,
        );
        assert_eq!(
            *patched.properties,
            snaps(&[
                ("cfg", VariantSnapshot::Bytes(vec![1, 2, 3])),
                (
                    "tags",
                    VariantSnapshot::Array(vec![VariantSnapshot::Int(1), VariantSnapshot::Int(2)])
                ),
                ("n", VariantSnapshot::Int(0)),
            ])
        );
        assert_ne!(patched.properties, base.properties);
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn untouched_tier1_board_encodes_nothing() {
        let mut base_objects: HashMap<String, Arc<SimObjectData>> = HashMap::new();
        base_objects.insert(
            "7".to_string(),
            Arc::new(SimObjectData {
                uid: "7".to_string(),
                groups: vec!["fire".to_string()],
                properties: Arc::new(snaps(&[
                    ("fuel", VariantSnapshot::Int(5)),
                    ("lit", VariantSnapshot::Bool(true)),
                ])),
            }),
        );
        let base = base_snapshot(
            snaps(&[
                ("hunger", VariantSnapshot::Int(3)),
                ("name", VariantSnapshot::Str("agent".to_string())),
                ("pos", VariantSnapshot::Float(1.5f64.to_bits())),
                ("held", VariantSnapshot::ObjectRef(42)),
                ("nothing", VariantSnapshot::Nil),
            ]),
            base_objects,
        );
        // Live content identical to base; nothing marked anywhere. Live values are
        // never encoded on this path, so their stand-ins only need matching keys.
        let agent_live = props(&[
            ("hunger", TestValue::Int(3)),
            ("name", TestValue::Int(0)),
            ("pos", TestValue::Int(0)),
            ("held", TestValue::Int(0)),
            ("nothing", TestValue::Int(0)),
        ]);
        let proxy_live = props(&[("fuel", TestValue::Int(5)), ("lit", TestValue::Int(1))]);
        let clean = HashSet::new();
        let live_objects = HashMap::from([("7".to_string(), live("7", &proxy_live, &clean))]);
        let count = Cell::new(0);
        let patched = patch(
            &base,
            &agent_live,
            &HashSet::new(),
            &live_objects,
            &HashSet::new(),
            &count,
        );
        assert_eq!(patched, base);
        assert_eq!(count.get(), 0);
        // Structural sharing: the untouched object keeps its Arc across the patch.
        assert!(
            Arc::ptr_eq(
                base.objects.get("7").expect("base has object 7"),
                patched.objects.get("7").expect("patched has object 7")
            ),
            "untouched objects stay ptr_eq after patch"
        );
        // A fully untouched patch shares the top-level maps too, not just their entries.
        assert!(
            Arc::ptr_eq(&base.properties, &patched.properties),
            "untouched properties map stays ptr_eq after patch"
        );
        assert!(
            Arc::ptr_eq(&base.objects, &patched.objects),
            "untouched objects map stays ptr_eq after patch"
        );
    }
}
