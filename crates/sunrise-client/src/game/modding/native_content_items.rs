//! Code item identities available while reading a package's content files.
use std::collections::BTreeMap;
use sunrise_client::mod_content::DeclaredContent;
use sunrise_mod_package::Manifest;

pub(super) fn owned_item_ids(
    manifest: &Manifest,
    declarations: &BTreeMap<&'static str, DeclaredContent>,
) -> Vec<String> {
    let own = declarations
        .get(manifest.id.as_str())
        .into_iter()
        .flat_map(|batch| batch.items.iter().map(|item| item.get().id.clone()));
    let from_host = manifest
        .served_by
        .as_deref()
        .and_then(|host| declarations.get(host))
        .into_iter()
        .flat_map(|batch| batch.items.iter())
        .map(|item| &item.get().id)
        .filter(|id| {
            id.split_once(':')
                .is_some_and(|(ns, _)| manifest.owned_namespaces().any(|owned| owned == ns))
        })
        .cloned();
    own.chain(from_host).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunrise_mod_package::{Content, content::Item};

    #[test]
    fn file_remainders_can_name_code_items_in_any_owned_namespace() {
        let manifest = Manifest::parse(
            r#"
            id = "porter"
            name = "Porter"
            version = "1.0.0"
            api = "0.2"
            side = "both"
            capabilities = ["registries"]
            namespaces = ["legacy"]
        "#,
        )
        .unwrap();
        let item = |id: &str| {
            Item {
                id: id.into(),
                template: "minecraft:stick".into(),
                ..Item::default()
            }
            .validate_local()
            .unwrap()
        };
        let batches = BTreeMap::from([
            (
                "porter",
                DeclaredContent {
                    items: vec![item("porter:spoon"), item("legacy:wrench")],
                    ..Default::default()
                },
            ),
            (
                "foreign",
                DeclaredContent {
                    items: vec![item("foreign:tool")],
                    ..Default::default()
                },
            ),
        ]);
        let declared = owned_item_ids(&manifest, &batches);
        assert_eq!(declared, ["porter:spoon", "legacy:wrench"]);
        let root = std::env::temp_dir().join(format!(
            "sunrise-owned-code-items-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let folder = root.join("data/legacy/sunrise/item");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("soup.json");
        std::fs::write(
            &file,
            br#"{"template":"minecraft:mushroom_stew","crafting_remainder":"legacy:wrench"}"#,
        )
        .unwrap();
        let content = Content::read_declaring(&root, &manifest, &declared).unwrap();
        assert_eq!(content.items[0].id, "legacy:soup");
        assert_eq!(
            content.items[0].crafting_remainder.as_deref(),
            Some("legacy:wrench")
        );
        std::fs::write(
            &file,
            br#"{"template":"minecraft:mushroom_stew","crafting_remainder":"foreign:tool"}"#,
        )
        .unwrap();
        let error = Content::read_declaring(&root, &manifest, &declared).unwrap_err();
        assert!(
            error.contains("foreign:tool") && error.contains("crafting_remainder"),
            "{error}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn served_content_remainders_see_only_host_items_in_served_namespaces() {
        let manifest = Manifest::parse(
            r#"
            id = "runes"
            name = "Runes"
            version = "1.0.0"
            api = "0.2"
            side = "both"
            capabilities = ["registries"]
            namespaces = ["runes_extra"]
            served_by = "horizon_java"
        "#,
        )
        .unwrap();
        let item = |id: &str| {
            Item {
                id: id.into(),
                template: "minecraft:stick".into(),
                ..Item::default()
            }
            .validate_local()
            .unwrap()
        };
        let batches = BTreeMap::from([
            (
                "horizon_java",
                DeclaredContent {
                    items: vec![
                        item("runes:bowl"),
                        item("runes_extra:jar"),
                        item("horizon_java:wrench"),
                        item("other_served:wand"),
                    ],
                    ..Default::default()
                },
            ),
            (
                "unrelated",
                DeclaredContent {
                    items: vec![item("runes:spoofed")],
                    ..Default::default()
                },
            ),
        ]);
        assert_eq!(
            owned_item_ids(&manifest, &batches),
            ["runes:bowl", "runes_extra:jar"]
        );
    }
}
