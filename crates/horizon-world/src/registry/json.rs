//! Java 26.3 compact/full block-state JSON codecs over a pinned registry.
use super::{BlockRegistry, BlockStateId};
use crate::compression::invalid;
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    io,
};

/// Property kinds accompany the imported state registry. Integer properties
/// parse Java decimal strings; enum and boolean properties use exact names.
pub struct BlockStateJsonCodec<'a> {
    registry: &'a BlockRegistry,
    integers: BTreeSet<(String, String)>,
    counts: HashMap<&'a str, usize>,
}
impl<'a> BlockStateJsonCodec<'a> {
    pub fn new(
        registry: &'a BlockRegistry,
        integers: BTreeSet<(String, String)>,
    ) -> io::Result<Self> {
        Self::with_validation(registry, integers, true)
    }

    /// The Cafe worldgen table derives integer properties directly from this
    /// same immutable registry, so validating every derived pair again only
    /// repeats work at startup.
    #[cfg(feature = "cafe-terrain-only")]
    pub(crate) fn new_for_cafe_terrain(
        registry: &'a BlockRegistry,
        integers: BTreeSet<(String, String)>,
    ) -> Self {
        Self::with_validation(registry, integers, false)
            .expect("trusted registry-derived integer properties")
    }

    fn with_validation(
        registry: &'a BlockRegistry,
        integers: BTreeSet<(String, String)>,
        validate: bool,
    ) -> io::Result<Self> {
        if validate {
            for (block, property) in &integers {
                let state = registry
                    .default_state(block)
                    .and_then(|id| registry.state(id));
                if !state.is_some_and(|state| state.properties.contains_key(property)) {
                    return Err(invalid("integer property is absent from block registry"));
                }
            }
        } else {
        }
        // Registry facts keep each block's states together. Count those runs
        // first so the common path hashes each block once, rather than once
        // for all 35k state IDs. The fallback preserves the codec for arbitrary
        // registries whose IDs interleave block names.
        let mut counts = HashMap::with_capacity(registry.defaults.len());
        let mut index = 0;
        let mut runs = 0;
        while index < registry.states.len() {
            let name = registry.states[index].name.as_str();
            let start = index;
            index += 1;
            while index < registry.states.len() && registry.states[index].name == name {
                index += 1;
            }
            counts.insert(name, index - start);
            runs += 1;
        }
        if runs != registry.defaults.len() {
            counts.clear();
            for state in &registry.states {
                *counts.entry(state.name.as_str()).or_insert(0) += 1;
            }
        }
        Ok(Self {
            registry,
            integers,
            counts,
        })
    }
    /// `full` selects FULL_CODEC, which requires the object representation.
    /// Invalid/missing properties keep their defaults, as vanilla's codec does.
    pub fn decode(&self, json: &str, full: bool) -> io::Result<BlockStateId> {
        let value: Value = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        let name = if !full && value.is_string() {
            value.as_str()
        } else {
            value.get("id").and_then(Value::as_str)
        }
        .ok_or_else(|| invalid("missing block identifier"))?;
        let (namespace, path) = name.split_once(':').unwrap_or(("minecraft", name));
        let name = format!(
            "{}:{path}",
            if namespace.is_empty() {
                "minecraft"
            } else {
                namespace
            }
        );
        let default = self
            .registry
            .default_state(&name)
            .ok_or_else(|| invalid("unknown block identifier"))?;
        let mut state = self.registry.state(default).unwrap().clone();
        if let Some(properties) = value.get("properties").and_then(Value::as_object) {
            for (key, raw) in properties {
                if !state.properties.contains_key(key) {
                    continue;
                }
                let Some(value) = raw.as_str() else {
                    continue;
                };
                let candidate = if self.integers.contains(&(name.clone(), key.clone())) {
                    let Some(value) = java_integer(value) else {
                        continue;
                    };
                    value.to_string()
                } else {
                    value.to_owned()
                };
                let previous = state.properties.insert(key.clone(), candidate).unwrap();
                if self.registry.id(&state).is_none() {
                    state.properties.insert(key.clone(), previous);
                }
            }
        }
        self.registry
            .id(&state)
            .ok_or_else(|| invalid("block state is absent from registry"))
    }
    pub fn encode(&self, id: BlockStateId, full: bool) -> io::Result<String> {
        let state = self
            .registry
            .state(id)
            .ok_or_else(|| invalid("unknown block state ID"))?;
        let value = if !full && self.registry.default_state(&state.name) == Some(id) {
            Value::String(state.name.clone())
        } else if self.counts.get(state.name.as_str()) == Some(&1) {
            serde_json::json!({"id":state.name})
        } else {
            serde_json::json!({"id":state.name,"properties":state.properties})
        };
        serde_json::to_string(&value).map_err(|e| invalid(e.to_string()))
    }
}

pub(crate) use horizon_core::text::parse_java_integer as java_integer;
