//! Tag-only item table construction.
use super::*;
impl Items {
    /// Only the block and item tags (and the enchantment registry, which the
    /// station rules and loot functions are built from), for tools that need
    /// tag membership.
    pub fn tags_only(root: &Path, overlay: &PackOverlay, defaults: &ItemDefaults) -> Result<Self> {
        let block_items = block_items(root)?;
        let (component_registry, entity_type_tags) =
            component_tables(root, overlay, LoadEntityTags::Prepared)?;
        let mut items = Self {
            component_registry,
            entity_type_tags,
            component_defaults: component_defaults(defaults)?,
            ..Self::default()
        };
        items.bind_tags(root, overlay, defaults, |id| block_items.contains_key(id))?;
        items.install_registries(
            crate::enchant::Enchantments::load_definitions(root, overlay, defaults)?,
            HashMap::new(),
        );
        load_component_damage_registry(root, overlay, &mut items)?;
        Ok(items)
    }

    /// Bind ingredient codecs to the world's frozen damage registry and reloaded tags.
    /// Do this before preparing recipes, alongside binding item damage predicates.
    pub fn bind_component_damage_catalog(
        &mut self,
        catalog: &sunrise_world::damage_source::Catalog,
    ) {
        let mut ids: Vec<_> = catalog
            .entries()
            .map(|(_, entry)| entry.name.clone())
            .collect();
        ids.sort();
        let registry = Arc::make_mut(&mut self.component_registry);
        registry.by_registry.insert(
            "minecraft:damage_type".into(),
            sunrise_client_core::registries::RegistryIds {
                ids,
                tags: catalog.tags().map(str::to_owned).collect(),
            },
        );
    }
}

/// Standalone catalogs have no world-owned snapshot; load their complete pack registry.
pub(super) fn load_component_damage_registry(
    root: &Path,
    overlay: &PackOverlay,
    items: &mut Items,
) -> Result<()> {
    if root.join("damage-catalog.json").exists() {
        let catalog = sunrise_world::damage_source::Catalog::load(root, overlay)?;
        items.bind_component_damage_catalog(&catalog);
    }
    Ok(())
}

/// Preserve the exact loaded prototype, including native default-component changes.
pub(super) fn component_defaults(
    defaults: &ItemDefaults,
) -> Result<Arc<HashMap<String, sunrise_core::nbt::Compound>>> {
    Ok(sunrise_world::loot::copy::component_defaults(defaults)?)
}

/// Catalogs also need Block.getItem for block-tag ingredients, without full block facts.
pub(super) fn block_items(root: &Path) -> Result<HashMap<String, String>> {
    #[derive(serde::Deserialize)]
    struct Registry {
        defaults: HashMap<String, usize>,
    }
    #[derive(serde::Deserialize)]
    struct Gameplay {
        states: Vec<State>,
    }
    #[derive(serde::Deserialize)]
    struct State {
        item: Option<String>,
    }
    let open = |name: &str| match fs::File::open(root.join(name)) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    };
    let (Some(registry), Some(gameplay)) = (open("facts.json")?, open("gameplay-facts.json")?)
    else {
        return Ok(HashMap::new());
    };
    let registry: Registry = serde_json::from_reader(std::io::BufReader::new(registry))?;
    let gameplay: Gameplay = serde_json::from_reader(std::io::BufReader::new(gameplay))?;
    registry
        .defaults
        .into_iter()
        .map(|(block, state)| {
            let item = gameplay
                .states
                .get(state)
                .ok_or("block default state is missing gameplay facts")?
                .item
                .as_deref()
                .unwrap_or("minecraft:air");
            Ok((block, item.to_owned()))
        })
        .collect()
}

pub(crate) enum LoadEntityTags<'a> {
    Prepared,
    Frozen(&'a Arc<[String]>),
    Bound(&'a Arc<EntityTypeTags>),
}

/// The entity membership and registry metadata used by ingredient component codecs.
pub(crate) fn component_tables(
    root: &Path,
    overlay: &PackOverlay,
    source: LoadEntityTags<'_>,
) -> Result<(
    Arc<sunrise_client_core::registries::Registries>,
    Arc<EntityTypeTags>,
)> {
    let mut registry = sunrise_client_core::registries::Registries::default();
    let path = root.join("command-registries.json");
    if path.exists() {
        registry.fill_from_export(&serde_json::from_slice(&fs::read(path)?)?);
    }
    let path = root.join("registries.json");
    if path.exists() {
        let json: Value = serde_json::from_slice(&fs::read(path)?)?;
        if let Some(entries) = json["minecraft:data_component_type"]["entries"].as_object() {
            registry
                .by_registry
                .entry("minecraft:data_component_type".into())
                .or_default()
                .ids = entries.keys().cloned().collect();
        }
    }
    let entity_type_tags = match source {
        LoadEntityTags::Prepared => {
            // The prepared entity table includes content-mod types, unlike the command export.
            let ids = match fs::read(root.join("entity-types.json")) {
                Ok(bytes) => EntityTypeTags::registry_ids(&serde_json::from_slice(&bytes)?)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Arc::from(registry.ids(EntityTypeTags::REGISTRY))
                }
                Err(error) => return Err(error.into()),
            };
            Arc::new(EntityTypeTags::load(root, overlay, ids)?)
        }
        LoadEntityTags::Frozen(ids) => Arc::new(EntityTypeTags::load(root, overlay, ids.clone())?),
        LoadEntityTags::Bound(tags) => tags.clone(),
    };
    let entities = registry
        .by_registry
        .entry(EntityTypeTags::REGISTRY.into())
        .or_default();
    entities.ids = entity_type_tags.ids().to_vec();
    // These are the current merged pack tags, not the export's stale tag names.
    entities.tags = entity_type_tags.ordered().keys().cloned().collect();
    entities.tags.sort();
    // The command export is vanilla-only. World recipe loads bind their active
    // frozen catalog; standalone catalogs load the full pack/mod catalog above.
    registry.damage_types.clear();
    registry.by_registry.insert(
        "minecraft:damage_type".into(),
        sunrise_client_core::registries::RegistryIds::default(),
    );
    Ok((Arc::new(registry), entity_type_tags))
}
