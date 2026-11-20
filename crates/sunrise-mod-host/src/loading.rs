//! Installed native and wasm code loading.
use mod_api::__private::game::Fields;
use std::path::Path;
use sunrise_mod_package::{
    Manifest,
    dependencies::{Overrides, Resolution, resolve_manifests},
    manifest::Side,
    mod_list,
};

fn resolve_startup(
    manifests: &[&Manifest],
    side: Side,
    game_dir: &Path,
) -> Result<Resolution, String> {
    // The native dynamic loader and compiled package inventory already enforce
    // native physical requirements. Only logical requirements reach this resolver.
    let overrides = Overrides::read(&game_dir.join(mod_list::DIR))?;
    resolve_manifests(manifests, side, &overrides).map_err(|error| error.to_string())
}

/// Load installed native and wasm code, register its dependency-ordered entries,
/// and start the engine. Keep the validated startup lease alive for the whole run.
pub fn main_with(
    mods: Vec<mod_api::ModEntry>,
    slots: &[crate::SlotListener],
    built: &'static [mod_api::__private::native::BuiltMod],
    startup: sunrise_client::entry::Startup,
) {
    // The stack policy follows the engine mods run on, and must be settled before any worker starts.
    if let Err(error) = sunrise_mod_wasm::install_stack_policy() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    sunrise_engine_slots::install_seal(mod_points::__private::seal)
        .expect("engine seal provider must be installed once before startup");
    // The title screen's Mods button: every game that loads mods has it, before the game seals
    // the title screen's buttons.
    if let Err(error) = sunrise_hooks::title::register(&crate::mods_button::MODS) {
        log::warn!("Sunrise: the title screen's Mods button did not register: {error}");
    }
    // Its label travels with it: the crate that supplies the row supplies its text.
    if let Err(error) = sunrise_hooks::lang::register(crate::mods_button::LANG) {
        log::warn!("Sunrise: the title screen's Mods label did not register: {error}");
    }
    // Platform instrumentation must precede registration and the first mod guard.
    if let Err(error) = mod_api::__private::initialize() {
        eprintln!("Sunrise: refusing startup because mod instrumentation is unavailable: {error}");
        std::process::exit(1);
    }
    if let Err(error) = sunrise_engine_slots::install_platform(sunrise_engine_slots::Platform {
        install_packages: mod_api::__private::install_packages,
        install_registration_data: mod_api::__private::install_registration_data,
        install_parts: mod_api::__private::install_parts,
        profiler_is_enabled: mod_api::__private::profiler::is_enabled,
        profiler_start: mod_api::__private::profiler::start,
        profiler_stop: mod_api::__private::profiler::stop,
        profiler_summary: mod_api::__private::profiler::summary,
        profiler_firefox_profile: mod_api::__private::profiler::firefox_profile,
    }) {
        eprintln!("Sunrise: {error}");
        std::process::exit(1);
    }
    let exe = std::env::current_exe();
    let native: Result<_, Box<dyn std::error::Error + Send + Sync>> = exe
        .as_ref()
        .map_err(|e| e.to_string().into())
        .and_then(|exe| {
            sunrise_mod_package::runtime::Runtime::load(exe, built).map_err(Into::into)
        });
    let native = match native {
        Ok(native) => native,
        Err(error) => {
            eprintln!("Sunrise: {error}");
            std::process::exit(1);
        }
    };
    mod_render::render::install();
    sunrise_client::entry::start(
        native,
        |native, launch| {
            sunrise_mod_wasm::set_game_build(crate::GAME_BUILD_ID);
            let mut wasm =
                sunrise_mod_wasm::load_installed(launch, &sunrise_client::logging::player_dir());
            let mut mods = mods;
            mods.extend(std::mem::take(&mut wasm.mods));
            // One installed dispatch order for both targets; ties retain bundle order.
            let code_manifests: Vec<_> = native
                .packages()
                .iter()
                .map(|p| &p.manifest)
                .chain(wasm.manifests.iter())
                .filter(|m| mods.iter().any(|entry| entry.fields().id == m.id))
                .collect();
            assert_eq!(
                code_manifests.len(),
                mods.len(),
                "loaded code has checked manifests"
            );
            let side = launch.installation_side();
            let code_ids: std::collections::HashSet<_> =
                mods.iter().map(|entry| entry.fields().id).collect();
            let settings = mod_list::ModList::load(&launch.context.game_dir);
            let content: Vec<_> = sunrise_mod_package::selection::content_packages(
                &launch.context.game_dir,
                &settings.enabled,
                &launch.context.dev_mods,
            )
            .collect();
            let manifests: Vec<_> = code_manifests
                .iter()
                .copied()
                .chain(
                    content
                        .iter()
                        .filter(|source| {
                            let manifest = &source.package.manifest;
                            let code = manifest.native.is_some() || manifest.wasm.is_some();
                            !code || !code_ids.contains(manifest.id.as_str())
                        })
                        .map(|source| &source.package.manifest),
                )
                .collect();
            let order = resolve_startup(&manifests, side, &launch.context.game_dir).unwrap_or_else(
                |error| {
                    eprintln!("Sunrise: mod load refused: {error}");
                    sunrise_client::logging::flush();
                    std::process::exit(1);
                },
            );
            for warning in &order.warnings {
                log::warn!("Sunrise: {warning}");
            }
            // What every level.dat this game saves records (`fml.LoadingModList`).
            sunrise_client::save::level_mods::set_loaded(Some(
                order
                    .order
                    .iter()
                    .filter(|&&i| code_ids.contains(manifests[i].id.as_str()))
                    .map(|&i| sunrise_client::save::level_mods::ModRecord {
                        id: manifests[i].id.clone(),
                        version: manifests[i].version.to_string(),
                    })
                    .collect(),
            ));
            let mut entries: std::collections::BTreeMap<_, _> = mods
                .into_iter()
                .map(|entry| (entry.fields().id, entry))
                .collect();
            let mods: Vec<_> = order
                .order
                .into_iter()
                .filter(|&i| code_ids.contains(manifests[i].id.as_str()))
                .map(|i| entries.remove(manifests[i].id.as_str()).unwrap())
                .collect();
            let ids: Vec<_> = mods.iter().map(|entry| entry.fields().id).collect();
            mod_api::__private::install_dependencies(order.effective);
            mod_api::__private::install(mods.into());
            for &id in &ids {
                for slot in slots.iter().filter(|slot| slot.id() == id) {
                    if let Err(error) = mod_points::register(slot) {
                        log::error!("Sunrise: {id}: {error}");
                        sunrise_client::logging::flush();
                        std::process::exit(1);
                    }
                }
            }
            (ids, wasm)
        },
        sunrise_client::entry::CodeRuntime {
            initialize: sunrise_mod_wasm::initialize,
            refusal: |id| {
                sunrise_mod_wasm::refusal(id).map(|reason| {
                use sunrise_client::text::Text;
                match reason {
                    sunrise_mod_wasm::Refusal::CompileTimeout { member, budget } => {
                        let source = match budget.threads {
                            Some(threads) => Text::translate_with_fallback("sunrise.wasm.compile_default", "default for %s threads", &[Text::literal(threads.to_string())]),
                            None => Text::translate_with_fallback("sunrise.wasm.compile_setting", "setting %s", &[Text::literal(sunrise_mod_wasm::compile_budget::SETTING)]),
                        };
                        Text::translate_with_fallback("sunrise.wasm.compile_timeout", "%s: compilation exceeded %s seconds (%s). Increase %s to allow more compile time.", &[
                            Text::literal(member),
                            Text::literal(budget.limit.as_secs_f64().to_string()),
                            source,
                            Text::literal(sunrise_mod_wasm::compile_budget::SETTING),
                        ]).plain()
                    }
                    sunrise_mod_wasm::Refusal::CompileSetting(error) => {
                        use sunrise_mod_wasm::compile_budget::{SETTING, SettingError};
                        match error {
                            SettingError::InvalidValue(value) => Text::translate_with_fallback(
                                "sunrise.wasm.compile_setting_invalid",
                                "Invalid %s=%s: expected positive finite seconds.",
                                &[Text::literal(SETTING), Text::literal(format!("{value:?}"))],
                            ).plain(),
                            SettingError::File { path, why } => Text::translate_with_fallback(
                                "sunrise.wasm.compile_setting_file",
                                "Cannot read %s from %s: %s",
                                &[Text::literal(SETTING), Text::literal(path.display().to_string()), Text::literal(why)],
                            ).plain(),
                        }
                    }
                    sunrise_mod_wasm::Refusal::CompileUnavailable { member, budget, why } => {
                        Text::translate_with_fallback(
                            "sunrise.wasm.compile_unavailable",
                            "%s: cannot enforce the %s second compilation budget: %s. Restore a private writable .sunrise-cache/wasm directory and a working compiler executable.",
                            &[Text::literal(member), Text::literal(budget.limit.as_secs_f64().to_string()), Text::literal(why)],
                        ).plain()
                    }
                    other => other.to_string(),
                }
            })
            },
            startup_refused: |id| {
                sunrise_mod_wasm::refused()
                    .iter()
                    .any(|(owner, _)| owner == id)
            },
        },
        startup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::__private::native::BuiltMod;
    use std::{fs, path::PathBuf};
    use sunrise_mod_package::{dependencies::OVERRIDES, runtime::Runtime};

    #[test]
    fn native_startup_requires_manifest_only_data_unless_dropped() {
        const BUILT: &[BuiltMod] = &[
            BuiltMod {
                id: "addon",
                version: "1.0.0",
                side: Side::Both,
            },
            BuiltMod {
                id: "data",
                version: "1.0.0",
                side: Side::Both,
            },
        ];
        struct Scratch(PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let root = Scratch(std::env::temp_dir().join(format!(
            "sunrise-native-startup-deps-{}",
            std::process::id()
        )));
        let package = root.0.join(mod_list::NATIVE_DIR).join("addon");
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(
            package.join("Cargo.toml"),
            "[package]\nname='addon'\nversion='1.0.0'\n",
        )
        .unwrap();
        fs::write(package.join("src/lib.rs"), "").unwrap();
        fs::write(package.join("sunrise-mod.toml"), format!(
            "id='addon'\nname='Addon'\nversion='1.0.0'\napi='0.2'\nside='both'\ncapabilities=['registries']\n[dependencies]\ndata='^1'\n[native]\ngame='26.3'\nsunrise='{}'\nmixins='{}'\n",
            "a".repeat(40), "b".repeat(40),
        )).unwrap();
        let executable = root.0.join("sunrise-client");
        let native = Runtime::load(&executable, &BUILT[..1]).unwrap();
        let manifests = [&native.packages()[0].manifest];
        assert_eq!(
            resolve_startup(&manifests, Side::Client, &root.0).unwrap_err(),
            "addon requires missing mod data"
        );

        let mods_dir = root.0.join(mod_list::DIR);
        fs::create_dir(&mods_dir).unwrap();
        fs::write(mods_dir.join(OVERRIDES), "[addon]\ndata='drop'\n").unwrap();
        let resolved = resolve_startup(&manifests, Side::Client, &root.0).unwrap();
        assert_eq!(resolved.order, [0]);
        assert!(!resolved.effective.permits("addon", "data"));

        // If the executable actually includes the dependency's code, its package
        // is required before logical policy can be applied, even with the drop.
        let error = Runtime::load(&executable, BUILT).unwrap_err();
        assert_eq!(error.refused.len(), 1);
        assert!(error.to_string().contains("data 1.0.0:"), "{error}");
    }
}
