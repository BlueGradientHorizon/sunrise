//! The required prepared-root boundary names every input it cannot use. These run in `verify`: they build
//! their own temporary root and read no prepared data.

use crate::test_resources::{require_inputs, unavailable_inputs};

fn scratch(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sunrise-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn every_missing_input_is_named() {
    let root = scratch("prepared-missing");
    std::fs::write(root.join("items.json"), "{}").unwrap();
    let found = unavailable_inputs(
        &root,
        &[&["items.json", "facts.json"], &["data/minecraft/recipe"]],
    );
    assert_eq!(
        found,
        ["facts.json is missing", "data/minecraft/recipe is missing"]
    );
    assert!(unavailable_inputs(&root, &[&["items.json"]]).is_empty());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
#[should_panic(expected = "facts.json is missing")]
fn the_boundary_panics_naming_the_missing_input() {
    let root = scratch("prepared-required");
    std::fs::write(root.join("items.json"), "{}").unwrap();
    require_inputs(root, &[&["items.json", "facts.json"]]);
}

#[test]
fn the_boundary_returns_a_root_whose_inputs_are_all_there() {
    let root = scratch("prepared-present");
    std::fs::write(root.join("items.json"), "{}").unwrap();
    assert_eq!(require_inputs(root.clone(), &[&["items.json"]]), root);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_root_that_is_not_there_names_each_input() {
    let root = scratch("prepared-absent").join("none");
    assert_eq!(
        unavailable_inputs(&root, &[&["items.json", "worldgen"]]),
        ["items.json is missing", "worldgen is missing"]
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_input_is_named_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch("prepared-unreadable");
    let file = root.join("facts.json");
    std::fs::write(&file, "{}").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    // A superuser reads it anyway; the check is about the diagnosis, so skip only that case.
    if std::fs::File::open(&file).is_ok() {
        std::fs::remove_dir_all(&root).unwrap();
        return;
    }
    let found = unavailable_inputs(&root, &[&["facts.json"]]);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].starts_with("facts.json is unreadable"),
        "{found:?}"
    );
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}
