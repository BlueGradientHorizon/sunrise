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
        let result =
            resolve::for_manifest(&index, &addon, &[local], "26.3", Side::Client, |_, _| true);
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
    let error = libraries.plan(&addon, "26.3", |_, _| true).err().unwrap();
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
fn fit_is_checked_newest_first_and_only_as_far_as_the_range_needs() {
    let addon = manifest("addon", "1.0.0", "library='^1'");
    let mut index = indexed(&[("library", serde_json::json!([]))]);
    let project = &mut index.projects[0];
    let row = project.versions[0].clone();
    project.versions = ["1.0.0", "2.0.0", "1.1.0"]
        .map(|version| Version {
            version: version.into(),
            ..row.clone()
        })
        .into();
    for (unfit, expected, probes) in [
        (Some("1.1.0"), "1.0.0", &["1.1.0", "1.0.0"][..]),
        (None, "1.1.0", &["1.1.0"][..]),
    ] {
        let calls = RefCell::new(Vec::new());
        let selected =
            resolve::for_manifest(&index, &addon, &[], "26.3", Side::Client, |_, row| {
                calls.borrow_mut().push(row.version.clone());
                Some(row.version.as_str()) != unfit
            })
            .unwrap();
        assert_eq!(selected.registry[0].1.version, expected);
        assert_eq!(calls.into_inner(), probes);
    }
}

#[test]
fn a_local_content_package_satisfies_a_required_edge_without_the_index() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("content");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        dir.join(sunrise_mod_package::manifest::MANIFEST),
        "id='content'\nname='Content'\nversion='1.0.0'\napi='0.2'\ncapabilities=['registries']\n",
    )
    .unwrap();
    let path = root.path().join("content.smod");
    std::fs::write(&path, Package::pack_directory(&dir).unwrap()).unwrap();
    let addon = manifest("addon", "1.0.0", "content='^1'");
    let libraries = Libraries::read(&addon, &[&path], Side::Client).unwrap();
    assert!(libraries.index.projects.is_empty());
    let selected = libraries
        .plan(&addon, "26.3", |_, _| panic!("no native candidate"))
        .unwrap();
    assert_eq!(selected.local, ["content"]);
    assert!(selected.registry.is_empty());
}

#[test]
fn the_install_planner_resolves_required_edges_and_explicit_optional_libraries() {
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
    assert!(index.left_out.is_empty(), "{:?}", index.left_out);
    let extra = manifest("extra", "1.1.0", "");
    for (supplied, expected) in [(None, &["local"][..]), (Some(&extra), &["extra", "local"])] {
        let locals: Vec<_> = std::iter::once(&local)
            .chain(supplied)
            .map(|manifest| resolve::LocalMod {
                manifest,
                sha256: None,
            })
            .collect();
        let selected =
            resolve::for_manifest(&index, &addon, &locals, "26.3", Side::Client, |_, _| true)
                .unwrap();
        assert_eq!(selected.local, expected);
        assert_eq!(
            selected
                .registry
                .iter()
                .map(|(p, _)| p.id.as_str())
                .collect::<Vec<_>>(),
            ["base", "bridge"]
        );
    }
}
