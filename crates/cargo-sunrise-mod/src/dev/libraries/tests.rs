use super::*;

fn manifest(id: &str, version: &str, dependencies: &str) -> Manifest {
    Manifest::parse(&format!(
        "id='{id}'\nname='Test Mod'\nversion='{version}'\napi='0.2'\ncapabilities=['registries']\n\
         [native]\ngame='26.3'\nsunrise='{}'\n[dependencies]\n{dependencies}",
        "a".repeat(40)
    ))
    .unwrap()
}

fn indexed(rows: &[(&str, serde_json::Value)]) -> Index {
    let projects: Vec<_> = rows.iter().map(|(id, deps)| serde_json::json!({
        "id": id, "kind": "native-mod", "name": id, "summary": "A library", "license": "MIT", "source": "https://example.org/source",
        "versions": [{"version": "1.2.0", "api": "native-1", "game": "26.3", "dependencies": deps,
            "file": {"name": format!("{id}.smod"), "size": 23, "url": "https://example.org/library.smod", "sha256": "b".repeat(64)}}]
    })).collect();
    Index::parse(&serde_json::json!({"schema": 1, "projects": projects}).to_string()).unwrap()
}

#[test]
fn a_local_library_wins_and_its_version_cannot_be_replaced() {
    let addon = manifest("addon", "1.0.0", "library='^1'");
    let index = indexed(&[("library", serde_json::json!([]))]);
    for (version, fits) in [("1.1.0", true), ("2.0.0", false)] {
        let local = manifest("library", version, "");
        let local = resolve::LocalMod {
            manifest: &local,
            sha256: None,
        };
        let result = resolve::for_manifest(
            &index,
            &addon,
            &[local],
            "26.3",
            Side::Client,
            true,
            |_, _| true,
        );
        if fits {
            let selected = result.unwrap();
            assert_eq!(selected.local, ["library"]);
            assert!(selected.registry.is_empty());
        } else {
            let error = result.err().unwrap();
            assert!(
                error.contains("library") && error.contains("addon needs ^1"),
                "{error}"
            );
        }
    }
}

#[test]
fn a_missing_requirement_names_the_mod_range_and_both_sources() {
    let addon = manifest("addon", "1.0.0", "missing_library='=1.2.3'");
    let libraries = Libraries {
        local: BTreeMap::new(),
        index: Index::default(),
        side: Side::Client,
    };
    let error = libraries.plan(&addon, "26.3").err().unwrap();
    for required in [
        "addon",
        "missing_library",
        "=1.2.3",
        "--with <path>",
        "Sunrise mod registry",
    ] {
        assert!(error.contains(required), "{error}");
    }
}

#[test]
fn the_install_planner_resolves_local_and_registry_edges_and_available_optional_libraries() {
    let addon = manifest(
        "addon",
        "1.0.0",
        "local='^1'\nmissing_optional={version='^1', optional=true}",
    );
    let local = manifest(
        "local",
        "1.1.0",
        "bridge='^1'\nserver_only={version='^1', side='server'}",
    );
    let index = indexed(&[
        (
            "bridge",
            serde_json::json!([
                {"id": "base", "version": "^1"}, {"id": "extra", "version": "^1", "optional": true}
            ]),
        ),
        ("base", serde_json::json!([])),
        ("extra", serde_json::json!([])),
    ]);
    let selected = resolve::for_manifest(
        &index,
        &addon,
        &[resolve::LocalMod {
            manifest: &local,
            sha256: None,
        }],
        "26.3",
        Side::Client,
        true,
        |_, _| true,
    )
    .unwrap();
    assert_eq!(selected.local, ["local"]);
    assert_eq!(
        selected
            .registry
            .iter()
            .map(|(p, _)| p.id.as_str())
            .collect::<Vec<_>>(),
        ["base", "bridge", "extra"]
    );
}
