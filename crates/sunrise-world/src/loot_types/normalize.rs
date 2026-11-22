//! Resolve predicate/item-modifier tags once, without entering codec-owned payloads.
use super::super::loot_tags::BoundTags as Tags;
use super::*;

impl super::super::LootData {
    /// Edit, normalize and bind private table replacements before the one publication.
    /// A refused replacement leaves this snapshot and its retained bindings intact.
    pub fn apply_native_table_loads(
        &mut self,
        context: &crate::native_load_context::LoadContext,
    ) -> io::Result<()> {
        let mut ids: Vec<_> = self.tables.ids().collect();
        ids.sort();
        let mut tables = HashMap::with_capacity(ids.len());
        for id in ids {
            let mut table = self.tables[id].clone();
            super::super::native_loot_table_load(context, id, &mut table);
            super::super::validate_loot_table(&table).map_err(|reason| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("native loot_table {id}: {reason}"),
                )
            })?;
            self.holder_sets
                .walk(&format!("native loot_table {id}"), &mut table, &self.tags)?;
            table_fields(&mut table, &self.tags, &self.types);
            tables.insert(id.to_owned(), table);
        }
        let tables: Arc<super::super::LootTables> = Arc::new(tables.into_iter().collect());
        let (predicates, modifiers, enchantment_effects) = if self.normalized {
            (
                self.predicates.clone(),
                self.modifiers.clone(),
                self.enchantments
                    .iter()
                    .map(|e| (e.name.clone(), e.effects.clone()))
                    .collect(),
            )
        } else {
            normalize_resources(self)
        };
        let bindings = self.types.bind_generation(
            tables.clone(),
            predicates.clone(),
            modifiers.clone(),
            enchantment_effects.clone(),
            loot_tag_subset(&self.tags),
        )?;
        self.tables = tables;
        self.predicates = predicates;
        self.modifiers = modifiers;
        for (enchantment, (_, effects)) in self.enchantments.iter_mut().zip(enchantment_effects) {
            enchantment.effects = effects;
        }
        self.bindings = bindings;
        self.normalized = true;
        self.refresh_copy_requirements();
        Ok(())
    }

    /// Normalize and bind tables, predicates, modifiers and enchantment equipment-drop
    /// requirements once into an immutable generation bundle.
    pub fn prepare_types(&mut self) -> io::Result<()> {
        let mut tables = HashMap::with_capacity(self.tables.len());
        for id in self.tables.ids() {
            let mut table = self.tables[id].clone();
            table_fields(&mut table, &self.tags, &self.types);
            tables.insert(id.to_owned(), table);
        }
        let tables: Arc<super::super::LootTables> = Arc::new(tables.into_iter().collect());
        let (predicates, modifiers, enchantment_effects) = normalize_resources(self);
        let bindings = self.types.bind_generation(
            tables.clone(),
            predicates.clone(),
            modifiers.clone(),
            enchantment_effects.clone(),
            loot_tag_subset(&self.tags),
        )?;
        self.tables = tables;
        self.predicates = predicates;
        self.modifiers = modifiers;
        for (enchantment, (_, effects)) in self.enchantments.iter_mut().zip(enchantment_effects) {
            enchantment.effects = effects;
        }
        self.bindings = bindings;
        self.normalized = true;
        self.refresh_copy_requirements();
        Ok(())
    }
}

fn normalize_resources(
    data: &super::super::LootData,
) -> (
    Arc<HashMap<String, Value>>,
    Arc<HashMap<String, Value>>,
    Vec<(String, Arc<Value>)>,
) {
    let mut predicates = (*data.predicates).clone();
    for value in predicates.values_mut() {
        typed(value, false, &data.tags, &data.types);
    }
    let mut modifiers = (*data.modifiers).clone();
    for value in modifiers.values_mut() {
        typed(value, true, &data.tags, &data.types);
    }
    let mut enchantment_effects = Vec::with_capacity(data.enchantments.len());
    for enchantment in &data.enchantments {
        let mut effects = (*enchantment.effects).clone();
        equipment_drop_requirements(&mut effects, &data.tags, &data.types);
        enchantment_effects.push((enchantment.name.clone(), Arc::new(effects)));
    }
    (
        Arc::new(predicates),
        Arc::new(modifiers),
        enchantment_effects,
    )
}

fn equipment_drop_requirements(effects: &mut Value, tags: &Tags, types: &Types) {
    for effect in effects
        .get_mut("minecraft:equipment_drops")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(req) = effect
            .get_mut("requirements")
            .filter(|value| !value.is_null())
        {
            typed(req, false, tags, types);
        }
    }
}

fn loot_tag_subset(tags: &Tags) -> HashMap<String, HashMap<String, Vec<String>>> {
    let mut subset = HashMap::new();
    for registry in ["predicate", "item_modifier"] {
        if let Some(entries) = tags.get(registry) {
            subset.insert(registry.to_owned(), entries.clone());
        }
    }
    subset
}

pub(super) fn normalize_table(value: &mut Value, tags: &Tags, types: &Types) {
    table_fields(value, tags, types);
}

pub(super) fn normalize_condition(value: &mut Value, tags: &Tags, types: &Types) {
    typed(value, false, tags, types);
}

fn table_fields(value: &mut Value, tags: &Tags, types: &Types) {
    fields(value, tags, types);
    for pool in value
        .get_mut("pools")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        fields(pool, tags, types);
        for entry in pool
            .get_mut("entries")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            container(entry, tags, types);
        }
    }
}
fn container(value: &mut Value, tags: &Tags, types: &Types) {
    fields(value, tags, types);
    for child in value
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        container(child, tags, types);
    }
    if value["type"] == "minecraft:loot_table" && value["value"].is_object() {
        table_fields(&mut value["value"], tags, types);
    }
}
fn fields(value: &mut Value, tags: &Tags, types: &Types) {
    if let Some(condition) = value.get_mut("condition") {
        typed(condition, false, tags, types);
    }
    if let Some(modifier) = value.get_mut("modifier") {
        typed(modifier, true, tags, types);
    }
}
fn typed(value: &mut Value, function: bool, tags: &Tags, types: &Types) {
    if let Some(id) = value.as_str() {
        if function && id.starts_with('#') {
            set(value, true, tags, types);
        } else if let Ok(id) = sunrise_core::identifier::Identifier::parse(id) {
            *value = Value::String(id.to_string());
        }
        return;
    }
    if let Some(id) = value.get("type").and_then(Value::as_str)
        && let Ok(id) = sunrise_core::identifier::Identifier::parse(id)
    {
        value["type"] = Value::String(id.to_string());
    }
    if function && value.is_array() {
        set(value, true, tags, types);
        return;
    }
    let kind = value["type"].as_str().unwrap_or("");
    let conditional = shapes::contains(kind, true) || types.conditional(kind);
    // Locate only schema-owned edges; scalar/text decoding belongs to the
    // binding pass, and registered payloads remain opaque.
    let children = shapes::children(value, function, kind)
        .into_iter()
        .map(|child| (child.kind, child.pointer))
        .collect::<Vec<_>>();
    for (kind, pointer) in children {
        if let Some(child) = value.pointer_mut(&pointer) {
            match kind {
                shapes::ChildKind::Function => typed(child, true, tags, types),
                shapes::ChildKind::Condition => typed(child, false, tags, types),
                shapes::ChildKind::Set(function) => set(child, function, tags, types),
                shapes::ChildKind::Table => {}
            }
        }
    }
    if function
        && conditional
        && let Some(condition) = value.get_mut("condition").filter(|value| !value.is_null())
    {
        typed(condition, false, tags, types);
    }
}

fn set(value: &mut Value, function: bool, tags: &Tags, types: &Types) {
    if let Some(tag) = value.as_str().and_then(|id| id.strip_prefix('#')) {
        let Ok(tag) = sunrise_core::identifier::Identifier::parse(tag) else {
            return;
        };
        let registry = if function {
            "item_modifier"
        } else {
            "predicate"
        };
        let ids = tags
            .get(registry)
            .and_then(|tags| tags.get(tag.to_string().as_str()));
        *value = Value::Array(
            ids.into_iter()
                .flatten()
                .cloned()
                .map(Value::String)
                .collect(),
        );
    } else if !value.is_array() {
        *value = Value::Array(vec![std::mem::take(value)]);
    }
    for term in value.as_array_mut().into_iter().flatten() {
        typed(term, function, tags, types);
    }
}
