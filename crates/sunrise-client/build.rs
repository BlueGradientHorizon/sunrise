//! Discovers game and ingredient test modules and generates the native mod API.

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = std::path::Path::new(&dir).join("../..");
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("crates/sunrise-client/src/game").display()
    );
    std::fs::write(
        out_dir.join("game_modules.rs"),
        sunrise_game_discovery::declarations(
            &root.join("crates/sunrise-client/src/game"),
            &["native_hooks", "host", "creatures"],
        )
        .expect("discovering game modules"),
    )
    .expect("writing game modules");
    let tests =
        root.join("crates/sunrise-client/src/crafting/ingredient/builtin_kinds_tests/families");
    println!("cargo:rerun-if-changed={}", tests.display());
    std::fs::write(
        out_dir.join("builtin_ingredient_test_modules.rs"),
        sunrise_game_discovery::folder_modules(&tests).expect("discovering ingredient tests"),
    )
    .expect("writing ingredient test modules");
    sunrise_mod_api_gen::generate(&root, &out_dir, sunrise_mod_api_gen::Target::Client)
        .expect("generating native mod API");
}
