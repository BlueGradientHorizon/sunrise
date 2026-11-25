//! Generates the native mod API (post-processed for Shape G rehearsal).
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn rewrite_host_generated(path: &Path) {
    let txt = fs::read_to_string(path).expect("read game_host_generated.rs");
    let mut out = String::with_capacity(txt.len() + 4096);
    for line in txt.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("impl ") && trimmed.ends_with(" for Game<'_> {") {
            let tr = trimmed
                .strip_prefix("impl ")
                .unwrap()
                .strip_suffix(" for Game<'_> {")
                .unwrap()
                .trim();
            if tr.ends_with("ClientHost") || tr.ends_with("ClientBackendMut") || tr.ends_with("ClientBackend") {
                out.push_str(&format!(
                    "impl {tr} for Game<'_, crate::game::client_side::ClientState> {{\n"
                ));
            } else if tr.ends_with("ServerHost") || tr.ends_with("BackendMut") {
                out.push_str(&format!(
                    "impl {tr} for Game<'_, crate::game::server_side::ServerState> {{\n"
                ));
            } else {
                out.push_str(&format!(
                    "impl<S: crate::game::Side> {tr} for Game<'_, S> {{\n"
                ));
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(path, out).expect("write rewritten game_host_generated.rs");
}

fn rewrite_fire_generated(path: &Path, fire_sides: &HashMap<String, String>) {
    let txt = fs::read_to_string(path).expect("read mod_api_fire_generated.rs");
    let mut out = String::with_capacity(txt.len() + 4096);
    for line in txt.lines() {
        if let Some(rest) = line.strip_prefix("impl Game < '_ > {") {
            let side = line
                .find("mod_api::dispatch_")
                .and_then(|idx| {
                    let s = &line[idx + "mod_api::".len()..];
                    let end = s.find('(')?;
                    fire_sides.get(&s[..end]).map(String::as_str)
                })
                .unwrap_or("both");
            match side {
                "server" => {
                    out.push_str("impl Game<'_, crate::game::server_side::ServerState> {");
                    out.push_str(rest);
                    out.push('\n');
                }
                "client" => {
                    out.push_str("impl Game<'_, crate::game::client_side::ClientState> {");
                    out.push_str(rest);
                    out.push('\n');
                }
                _ => {
                    out.push_str("impl<S: crate::game::Side> Game<'_, S> {");
                    out.push_str(rest);
                    out.push('\n');
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(path, out).expect("write rewritten mod_api_fire_generated.rs");
}

fn parse_fire_sides() -> HashMap<String, String> {
    let txt = fs::read_to_string("/projects/.hz-tmp/css-e2e/stage-d/switch-real/fire_sides.json")
        .expect("read fire_sides.json");
    let mut map = HashMap::new();
    for line in txt.lines() {
        let line = line.trim().trim_end_matches(',');
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().trim_matches('"');
            let v = v.trim().trim_matches('"');
            if !k.is_empty() && !v.is_empty() {
                map.insert(k.to_owned(), v.to_owned());
            }
        }
    }
    map
}

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = std::path::Path::new(&dir).join("../..");
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    sunrise_mod_api_gen::generate(&root, &out_dir, sunrise_mod_api_gen::Target::Client)
        .expect("generating native mod API");
    let fire_sides = parse_fire_sides();
    rewrite_host_generated(&out_dir.join("game_host_generated.rs"));
    rewrite_fire_generated(&out_dir.join("mod_api_fire_generated.rs"), &fire_sides);
    println!("cargo::rustc-check-cfg=cfg(sunrise_parity_fixtures)");
}
