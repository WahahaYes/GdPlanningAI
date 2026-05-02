//! Requirement and provision specs for planner-readable dependency chaining.
//!
//! These are deserialized from built-in Godot wrapper classes such as
//! `RequirementSpec.binding_exists(...)` and `ProvisionSpec.fact(...)`.

use crate::snapshot::VariantSnapshot;
use godot::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum RequirementSpec {
    BindingExists {
        binding_name: String,
    },
    BindingEquals {
        binding_name: String,
        value: VariantSnapshot,
    },
    BindingInSet {
        binding_name: String,
        set_name: String,
    },
    Fact {
        fact_name: String,
        args: Vec<VariantSnapshot>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProvisionSpec {
    Binding {
        binding_name: String,
        value: VariantSnapshot,
    },
    Fact {
        fact_name: String,
        args: Vec<VariantSnapshot>,
    },
}

impl RequirementSpec {
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let kind = dict
            .get("kind")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        match kind.as_str() {
            "binding_exists" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::BindingExists { binding_name })
            }
            "binding_equals" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                let value = dict.get("value")?;
                Some(Self::BindingEquals {
                    binding_name,
                    value: VariantSnapshot::from_variant(&value),
                })
            }
            "binding_in_set" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                let set_name = dict
                    .get("set_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::BindingInSet {
                    binding_name,
                    set_name,
                })
            }
            "fact" => {
                let fact_name = dict
                    .get("fact_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::Fact {
                    fact_name,
                    args: extract_variant_snapshots(dict, "args"),
                })
            }
            _ => {
                log_warn!("RequirementSpec: unrecognised kind '{}'", kind);
                None
            }
        }
    }
}

impl ProvisionSpec {
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let kind = dict
            .get("kind")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        match kind.as_str() {
            "binding" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                let value = dict.get("value")?;
                Some(Self::Binding {
                    binding_name,
                    value: VariantSnapshot::from_variant(&value),
                })
            }
            "fact" => {
                let fact_name = dict
                    .get("fact_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::Fact {
                    fact_name,
                    args: extract_variant_snapshots(dict, "args"),
                })
            }
            _ => {
                log_warn!("ProvisionSpec: unrecognised kind '{}'", kind);
                None
            }
        }
    }
}

pub fn requirements_satisfied(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
) -> bool {
    requirements
        .iter()
        .all(|requirement| requirement_satisfied(requirement, provisions))
}

pub fn requirement_satisfied(requirement: &RequirementSpec, provisions: &[ProvisionSpec]) -> bool {
    provisions
        .iter()
        .any(|provision| provision_satisfies_requirement(provision, requirement))
}

pub fn provisions_satisfy_any_requirement(
    provisions: &[ProvisionSpec],
    requirements: &[RequirementSpec],
) -> bool {
    requirements
        .iter()
        .any(|requirement| requirement_satisfied(requirement, provisions))
}

pub fn remove_satisfied_requirements(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
) -> Vec<RequirementSpec> {
    requirements
        .iter()
        .filter(|requirement| !requirement_satisfied(requirement, provisions))
        .cloned()
        .collect()
}

pub fn extend_unique_requirements(
    requirements: &mut Vec<RequirementSpec>,
    additional: &[RequirementSpec],
) {
    for requirement in additional {
        if !requirements.contains(requirement) {
            requirements.push(requirement.clone());
        }
    }
}

pub fn extend_unique_provisions(provisions: &mut Vec<ProvisionSpec>, additional: &[ProvisionSpec]) {
    for provision in additional {
        if !provisions.contains(provision) {
            provisions.push(provision.clone());
        }
    }
}

fn provision_satisfies_requirement(
    provision: &ProvisionSpec,
    requirement: &RequirementSpec,
) -> bool {
    match (provision, requirement) {
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                ..
            },
            RequirementSpec::BindingExists { binding_name },
        ) => provided_name == binding_name,
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                value: provided_value,
            },
            RequirementSpec::BindingEquals {
                binding_name,
                value,
            },
        ) => provided_name == binding_name && provided_value == value,
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                ..
            },
            RequirementSpec::BindingInSet { binding_name, .. },
        ) => provided_name == binding_name,
        (
            ProvisionSpec::Fact {
                fact_name: provided_name,
                args: provided_args,
            },
            RequirementSpec::Fact { fact_name, args },
        ) => provided_name == fact_name && provided_args == args,
        _ => false,
    }
}

fn extract_variant_snapshots(dict: &VarDictionary, key: &str) -> Vec<VariantSnapshot> {
    dict.get(key)
        .and_then(|v| {
            v.try_to::<Array<Variant>>().ok().or_else(|| {
                v.try_to::<VarArray>().ok().map(|arr| {
                    let mut typed = Array::<Variant>::new();
                    for item in arr.iter_shared() {
                        typed.push(&item);
                    }
                    typed
                })
            })
        })
        .map(|arr| {
            arr.iter_shared()
                .map(|value| VariantSnapshot::from_variant(&value))
                .collect()
        })
        .unwrap_or_default()
}
