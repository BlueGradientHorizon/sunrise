//! A library is compiled once and shared: its module must be the same bytes whether its author builds it alone or a
//! dependent's group builds it, and that holds only while every crate in the library's closure is built with the same
//! features in both. Cargo unifies features across a workspace, so a crate a dependent brings (the points' own
//! dependencies, say) can switch on a feature of a crate the library also uses; this test names such a crate.
//! It reads Cargo's resolution only and compiles nothing.
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// `(package, enabled features)` of every crate `package` builds for the wasm target, in `workspace`'s resolution.
fn closure(workspace: &Path, package: &str) -> BTreeSet<String> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or("cargo".into()))
        .args([
            "tree",
            "--offline",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .args([
            "-e",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p} [{f}]",
            "-p",
            package,
        ])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}: {}",
        workspace.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        // The package's own path differs between the workspaces; its dependencies are what is compared.
        .filter(|line| !line.contains(" (/"))
        .map(|line| line.trim_end_matches(" (*)").to_owned())
        .collect()
}

#[test]
#[ignore = "daily"]
fn a_library_is_built_with_the_same_features_in_its_dependents_workspace() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/library-fixture");
    let alone = closure(&fixtures.join("library"), "wasm-test-library");
    let grouped = closure(&fixtures.join("dependent"), "wasm-test-library");
    let differing: Vec<_> = alone.symmetric_difference(&grouped).collect();
    assert!(
        differing.is_empty(),
        "the library's dependencies are built with other features beside its dependent (alone / grouped): {differing:#?}"
    );
}
