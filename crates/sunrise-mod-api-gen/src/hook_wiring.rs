//! The source-only CI check for catalog targets. Build generation never calls the check.
use crate::{Catalog, Hook, catalog::DispatchStyle, dispatch, free_function_source, source};
use mod_source::{
    index::uses,
    modules::{ModuleFile, test_only, walk_module_files},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use syn::{Block, Expr, Item, visit::Visit};

/// Check that each declared hook target reaches its adapter or dispatcher. Never writes sources.
pub fn check_hook_wiring(root: &Path) -> Result<(), String> {
    check(root, &Catalog::load(root)?.hooks)
}

pub(super) fn check(root: &Path, hooks: &[Hook]) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("{}: {error}", root.display()))?;
    let mut sources = Sources {
        root: &root,
        files: BTreeMap::new(),
        modules: BTreeMap::new(),
    };
    let errors: Vec<_> = hooks
        .iter()
        .filter_map(|hook| {
            check_target(&mut sources, hook)
                .err()
                .map(|error| format!("{} ({}): {error}", hook.name, hook.target))
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// Index packages by Cargo's library roots (including client-lib's out-of-package root).
/// Keep parsed bodies only for modules the check inspects.
struct Sources<'a> {
    root: &'a Path,
    files: BTreeMap<&'static str, Result<Vec<ModuleFile>, String>>,
    modules: BTreeMap<(&'static str, String), Vec<Item>>,
}

impl Sources<'_> {
    fn items(&mut self, package: &'static str, module: &str) -> Result<&[Item], String> {
        let key = (package, module.to_owned());
        if !self.modules.contains_key(&key) {
            let root = self.root;
            let parse = |path: &Path, text: &str| {
                // The vocabulary root declares some modules in its existing export fragments.
                let parsed = if package == "sunrise-mod-vocab" {
                    syn::parse_file(&crate::exports::expand(root, text)?)
                } else {
                    syn::parse_file(text)
                };
                parsed.map_err(|error| format!("{}: {error}", path.display()))
            };
            let files = self.files.entry(package).or_insert_with(|| {
                let (_, lib) = source::crate_root(root, package)?;
                let dir = lib
                    .parent()
                    .expect("a library file has a folder")
                    .to_owned();
                walk_module_files(package.to_owned(), lib, dir, false, false, parse)
            });
            let mut items = Vec::new();
            let path = if module.is_empty() {
                package.to_owned()
            } else {
                format!("{package}::{module}")
            };
            for file in files
                .as_ref()
                .map_err(Clone::clone)?
                .iter()
                .filter(|file| file.module == path)
            {
                let text = fs::read_to_string(&file.path)
                    .map_err(|error| format!("{}: {error}", file.path.display()))?;
                items.extend(parse(&file.path, &text)?.items);
            }
            if items.is_empty() && !module.is_empty() {
                let (parent, name) = module.rsplit_once("::").unwrap_or(("", module));
                let parents = self.items(package, parent)?;
                for inner in source::inline_module_items(parents, name) {
                    items.extend_from_slice(inner);
                }
            }
            self.modules.insert(key.clone(), items);
        }
        Ok(&self.modules[&key])
    }

    /// Follow an installed named adapter to its function, including explicit game re-exports.
    fn named_adapter_calls(
        &mut self,
        package: &'static str,
        module: &str,
        path: &syn::Path,
        dispatch: &str,
    ) -> bool {
        let mut parts = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        let mut scope = module.to_owned();
        let mut seen = BTreeSet::new();
        loop {
            let Some((name, rest)) = parts.split_first() else {
                return false;
            };
            match name.as_str() {
                "crate" => scope.clear(),
                "self" => {}
                "super" => {
                    let end = scope.rfind("::").unwrap_or(0);
                    scope.truncate(end);
                }
                _ => {
                    let Ok(items) = self.items(package, &scope) else {
                        return false;
                    };
                    if rest.is_empty() {
                        let mut functions = functions(items, name).peekable();
                        if functions.peek().is_some() {
                            return functions.all(|body| calls(body, None, &[dispatch]));
                        }
                    }
                    let mut imports = items.iter().filter_map(|item| {
                        if let Item::Use(item) = item
                            && !test_only(&item.attrs)
                        {
                            let mut imported = None;
                            uses(&item.tree, &mut Vec::new(), &mut |path, binding| {
                                if binding.as_ref() == Some(name) {
                                    imported = Some(path);
                                }
                            });
                            imported
                        } else {
                            None
                        }
                    });
                    if let Some(import) = imports.next() {
                        if imports.next().is_some() || !seen.insert((scope.clone(), name.clone())) {
                            return false;
                        }
                        parts = import
                            .into_iter()
                            .chain(parts.into_iter().skip(1))
                            .collect();
                        continue;
                    }
                    if rest.is_empty() {
                        return false;
                    }
                    if !scope.is_empty() {
                        scope.push_str("::");
                    }
                    scope.push_str(name);
                }
            }
            parts.remove(0);
        }
    }
}

fn functions<'a>(items: &'a [Item], name: &'a str) -> impl Iterator<Item = &'a Block> {
    items.iter().filter_map(move |item| match item {
        Item::Fn(function) if function.sig.ident == name && !test_only(&function.attrs) => {
            Some(function.block.as_ref())
        }
        _ => None,
    })
}

const NATIVE_HOOKS: &str = "sunrise_client::game::native_hooks::";

fn check_target(sources: &mut Sources<'_>, hook: &Hook) -> Result<(), String> {
    let (package, module, owner, target) = if let Some(path) =
        hook.target.strip_prefix(NATIVE_HOOKS)
    {
        let (module_and_owner, target) = path.rsplit_once("::").ok_or("expected a method")?;
        let (module, owner) = module_and_owner
            .rsplit_once("::")
            .ok_or("expected a method owner")?;
        (
            "sunrise-client",
            format!("game::native_hooks::{module}"),
            Some(owner),
            target,
        )
    } else {
        let (package, path) = free_function_source(&hook.target).ok_or("unknown target crate")?;
        let (module, target) = path.rsplit_once("::").ok_or("expected a free function")?;
        (package, module.to_owned(), None, target)
    };
    let items = sources.items(package, &module)?;
    let bodies: Vec<&Block> = if let Some(owner) = owner {
        items
            .iter()
            .filter_map(|item| match item {
                Item::Impl(item)
                    if item.trait_.is_none()
                        && !test_only(&item.attrs)
                        && source::owner(&item.self_ty) == owner =>
                {
                    Some(&item.items)
                }
                _ => None,
            })
            .flatten()
            .filter_map(|item| match item {
                syn::ImplItem::Fn(method)
                    if method.sig.ident == target && !test_only(&method.attrs) =>
                {
                    Some(&method.block)
                }
                _ => None,
            })
            .collect()
    } else {
        functions(items, target).collect()
    };
    if bodies.is_empty() {
        return Err("target definition not found".into());
    }
    let name = &hook.name;
    if dispatch::is_render(hook) {
        // The raw target is an intentionally empty replacement slot. Start at the engine's
        // bound renderer callback, not a helper elsewhere in the same render module.
        let adapter = format!("generated::fire_{name}");
        return check_slot(
            sources,
            SlotTable {
                package,
                module: &module,
                installer: "install",
                install: "engine::install",
                slots: "engine::Slots",
            },
            name,
            &adapter,
        );
    }
    if hook.target == crate::catalog::EFFECT_TICK_TARGET {
        // The shared effect dispatcher invokes each Part method itself.
        return require_calls(&bodies, Some(name), &[]);
    }
    if owner.is_some() && hook.dispatch != DispatchStyle::Direct {
        let fire = format!("fire_{name}");
        return require_calls(&bodies, Some(&fire), &[&format!("Self::{fire}")]);
    }
    let direct = format!("mod_api::dispatch_{name}");
    if package == "sunrise-mod-vocab" {
        return require_calls(&bodies, None, &[&format!("crate::dispatch_{name}")]);
    }
    let (bridge, installer, install, slots) = match package {
        "sunrise-client-lib" => (
            "native_client_hooks",
            "install_client_hooks",
            "crate::native_client_hooks::install",
            "crate::native_client_hooks::NativeClientHooks",
        ),
        "sunrise-world" => (
            "native_load_hooks",
            "start",
            "sunrise_world::native_load_hooks::install",
            "sunrise_world::native_load_hooks::NativeLoadHooks",
        ),
        _ => return require_calls(&bodies, None, &[&direct]),
    };
    let wrapper = format!("crate::{bridge}::{name}");
    let needs_slot = bodies.iter().any(|body| calls(body, None, &[&wrapper]));
    if package == "sunrise-client-lib" {
        require_calls(&bodies, None, &[&direct, &wrapper])?;
    } else {
        require_calls(&bodies, None, &[&wrapper])?;
    }
    if needs_slot {
        // Calling a bridge is insufficient if the bridge never invokes its installed field.
        let mut wrappers = functions(sources.items(package, bridge)?, name).peekable();
        if wrappers.peek().is_none()
            || !wrappers.all(|body| {
                find_expr(body, |expr| {
                    let Expr::Call(call) = expr else { return None };
                    let Expr::Field(field) = production_expr(&call.func)? else {
                        return None;
                    };
                    matches!(&field.member, syn::Member::Named(member) if member == name)
                        .then_some(())
                })
                .is_some()
            })
        {
            return Err(format!(
                "{bridge}::{name} does not invoke its callback slot"
            ));
        }
        check_slot(
            sources,
            SlotTable {
                package: "sunrise-client",
                module: "entry",
                installer,
                install,
                slots,
            },
            name,
            &direct,
        )?;
    }
    Ok(())
}

fn require_calls(bodies: &[&Block], method: Option<&str>, paths: &[&str]) -> Result<(), String> {
    if bodies.iter().any(|body| !calls(body, method, paths)) {
        let expected = method
            .into_iter()
            .chain(paths.iter().copied())
            .collect::<Vec<_>>()
            .join(" or ");
        return Err(format!("target does not call {expected}"));
    }
    Ok(())
}

/// Where the executable binds a table of engine callbacks.
struct SlotTable<'a> {
    package: &'static str,
    module: &'a str,
    installer: &'a str,
    install: &'a str,
    slots: &'a str,
}

/// Find the actual slot initializer passed to `install`, then inspect only that callback.
fn check_slot(
    sources: &mut Sources<'_>,
    SlotTable {
        package,
        module,
        installer,
        install,
        slots,
    }: SlotTable<'_>,
    name: &str,
    dispatch: &str,
) -> Result<(), String> {
    let callback = functions(sources.items(package, module)?, installer)
        .find_map(|body| {
            find_expr(body, |expr| {
                let Expr::Call(call) = expr else { return None };
                if !path_is(function_path(&call.func)?, install) {
                    return None;
                }
                let Expr::Struct(table) = production_expr(call.args.first()?)? else {
                    return None;
                };
                if !path_is(&table.path, slots) {
                    return None;
                }
                table.fields.iter().find_map(|field| {
                    (matches!(&field.member, syn::Member::Named(member) if member == name)
                        && !test_only(&field.attrs))
                    .then_some(&field.expr)
                })
            })
        })
        .cloned()
        .ok_or_else(|| format!("{module}::{installer} does not install {slots}::{name}"))?;
    let mut reaches = |path: &syn::Path| {
        path_is(path, dispatch) || sources.named_adapter_calls(package, module, path, dispatch)
    };
    let wired = match production_expr(&callback) {
        Some(Expr::Path(path)) if path.qself.is_none() => reaches(&path.path),
        Some(callback @ Expr::Closure(_)) => {
            let mut check = Expressions {
                select: |expr: &Expr| {
                    let Expr::Call(call) = expr else { return None };
                    reaches(function_path(&call.func)?).then_some(())
                },
                found: None,
            };
            check.visit_expr(callback);
            check.found.is_some()
        }
        _ => false,
    };
    if !wired {
        return Err(format!(
            "{module}::{installer}: installed {slots}::{name} does not call {dispatch}"
        ));
    }
    Ok(())
}

fn production_expr(expr: &Expr) -> Option<&Expr> {
    if test_only(source::expr_attrs(expr)) {
        return None;
    }
    match expr {
        Expr::Paren(paren) => production_expr(&paren.expr),
        Expr::Group(group) => production_expr(&group.expr),
        _ => Some(expr),
    }
}

fn function_path(expr: &Expr) -> Option<&syn::Path> {
    match production_expr(expr)? {
        Expr::Path(path) if path.qself.is_none() => Some(&path.path),
        _ => None,
    }
}

fn path_is(path: &syn::Path, expected: &str) -> bool {
    let mut names = expected.split("::");
    path.segments
        .iter()
        .all(|segment| names.next().is_some_and(|name| segment.ident == name))
        && names.next().is_none()
}

fn calls(block: &Block, method: Option<&str>, paths: &[&str]) -> bool {
    has_call_matching(block, |call| match call {
        Callee::Method(called) => method.is_some_and(|method| called == method),
        Callee::Function(called) => paths.iter().any(|path| path_is(called, path)),
    })
}

pub(super) enum Callee<'ast> {
    Method(&'ast syn::Ident),
    Function(&'ast syn::Path),
}

pub(super) fn has_call_matching<'ast>(
    block: &'ast Block,
    matches: impl Fn(Callee<'ast>) -> bool,
) -> bool {
    find_expr(block, |expr| {
        let callee = match expr {
            Expr::MethodCall(call) => Callee::Method(&call.method),
            Expr::Call(call) => Callee::Function(function_path(&call.func)?),
            _ => return None,
        };
        matches(callee).then_some(())
    })
    .is_some()
}

fn find_expr<'ast, T>(
    block: &'ast Block,
    select: impl FnMut(&'ast Expr) -> Option<T>,
) -> Option<T> {
    let mut check = Expressions {
        select,
        found: None,
    };
    check.visit_block(block);
    check.found
}

struct Expressions<F, T> {
    select: F,
    found: Option<T>,
}

impl<'ast, F: FnMut(&'ast Expr) -> Option<T>, T> Visit<'ast> for Expressions<F, T> {
    // An unrelated local function's body does not wire its enclosing hook.
    fn visit_item(&mut self, _: &'ast Item) {}

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if self.found.is_none() && !test_only(source::expr_attrs(expr)) {
            self.found = (self.select)(expr);
            if self.found.is_none() {
                syn::visit::visit_expr(self, expr);
            }
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if !test_only(&local.attrs) {
            syn::visit::visit_local(self, local);
        }
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if !test_only(&arm.attrs) {
            syn::visit::visit_arm(self, arm);
        }
    }

    fn visit_field_value(&mut self, field: &'ast syn::FieldValue) {
        if !test_only(&field.attrs) {
            syn::visit::visit_field_value(self, field);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Side;

    fn fixture(root: &Path, path: &str, text: &str) {
        let path = root.join("crates").join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const PICKUP: &str = "sunrise-client/src/game/native_hooks/pickup.rs";

    fn pickup(root: &Path, declarations: &str) -> Hook {
        fixture(root, "sunrise-client/Cargo.toml", "");
        fixture(root, "sunrise-client/src/lib.rs", "mod game;");
        fixture(root, "sunrise-client/src/game.rs", "mod native_hooks;");
        fixture(
            root,
            "sunrise-client/src/game/native_hooks/mod.rs",
            declarations,
        );
        let mut hook = Hook::sample(
            "sunrise_client::game::native_hooks::pickup::Game<'_>::native_item_picked_up",
            Side::Server,
        );
        hook.name = "item_picked_up".into();
        hook
    }

    #[test]
    fn wiring_check_requires_a_call_in_the_declared_target() {
        let root = tempfile::tempdir().unwrap();
        let mut hook = pickup(root.path(), "mod pickup;");
        for (dispatch, body, expected) in [
            (DispatchStyle::Flat, "self.fire_item_picked_up(item);", true),
            (
                DispatchStyle::Flat,
                "let _ = (item); // self.fire_item_picked_up(item);\n",
                false,
            ),
            (
                DispatchStyle::Flat,
                "fn unused() { Self::fire_item_picked_up(item); }",
                false,
            ),
            (
                DispatchStyle::Flat,
                "mod_api::dispatch_item_picked_up(item);",
                false,
            ),
            (
                DispatchStyle::Direct,
                "other::dispatch_item_picked_up(item);",
                false,
            ),
            (
                DispatchStyle::Direct,
                "mod_api::dispatch_item_picked_up(item);",
                true,
            ),
        ] {
            let text = format!(
                "impl Game<'_> {{ fn native_item_picked_up(&self, item: u32) {{ {body} }} fn unrelated(&self) {{ self.fire_item_picked_up(1); }} }}"
            );
            fixture(root.path(), PICKUP, &text);
            hook.dispatch = dispatch;
            let result = check(root.path(), std::slice::from_ref(&hook));
            assert_eq!(result.is_ok(), expected, "{body}: {result:?}");
            assert_eq!(
                fs::read_to_string(root.path().join("crates").join(PICKUP)).unwrap(),
                text
            );
        }
    }

    #[test]
    fn test_only_pickup_calls_do_not_wire_the_production_target() {
        let root = tempfile::tempdir().unwrap();
        let hook = pickup(root.path(), "mod pickup;");
        for (body, wired) in [
            ("#[cfg(test)] { self.fire_item_picked_up(item); }", false),
            ("#[cfg(test)] self.fire_item_picked_up(item);", false),
            (
                "#[cfg(test)] let _test = self.fire_item_picked_up(item);",
                false,
            ),
            (
                "#[cfg(all(test, unix))] { self.fire_item_picked_up(item); }",
                false,
            ),
            ("at_pickup(|| self.fire_item_picked_up(item));", true),
        ] {
            fixture(
                root.path(),
                PICKUP,
                &format!(
                    "impl Game<'_> {{ fn native_item_picked_up(&self, item: u32) {{ {body} }} }}"
                ),
            );
            let result = check(root.path(), std::slice::from_ref(&hook));
            assert_eq!(result.is_ok(), wired, "{body}: {result:?}");
        }
    }

    #[test]
    fn pickup_uses_its_declared_path_or_inline_body() {
        let root = tempfile::tempdir().unwrap();
        let native = "sunrise-client/src/game/native_hooks";
        let hook = pickup(
            root.path(),
            "#[cfg(test)] mod pickup; #[path = \"pickup/live.rs\"] mod pickup;",
        );
        fixture(
            root.path(),
            PICKUP,
            "impl Game<'_> { fn native_item_picked_up(&self) {} }",
        );
        let wired =
            "impl Game<'_> { fn native_item_picked_up(&self) { self.fire_item_picked_up(); } }";
        fixture(root.path(), &format!("{native}/pickup/live.rs"), wired);
        check(root.path(), std::slice::from_ref(&hook)).unwrap();
        fixture(
            root.path(),
            &format!("{native}/mod.rs"),
            &format!("mod pickup {{ {wired} }}"),
        );
        check(root.path(), &[hook]).unwrap();
    }

    #[test]
    fn render_check_requires_the_installed_slot_not_an_unrelated_caller() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), "sunrise-mod-render/Cargo.toml", "");
        fixture(root.path(), "sunrise-mod-render/src/lib.rs", "mod render;");
        let mut hook = Hook::sample("mod_render::render::render_needs", Side::Client);
        hook.name = "render_needs".into();
        for (callback, wired) in [
            ("|needs| { generated::fire_render_needs(needs); }", true),
            ("|_needs| {}", false),
        ] {
            fixture(
                root.path(),
                "sunrise-mod-render/src/render.rs",
                &format!(
                    r"
                fn render_needs(_: &mut RenderNeeds) {{}}
                fn needs() {{ generated::fire_render_needs(&mut RenderNeeds::default()); }}
                fn install() {{
                    use sunrise_client_render::render_slots as engine;
                    engine::install(engine::Slots {{ render_needs: {callback} }});
                }}
            "
                ),
            );
            let result = check(root.path(), std::slice::from_ref(&hook));
            if wired {
                result.unwrap();
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    "render_needs (mod_render::render::render_needs): render::install: installed engine::Slots::render_needs does not call generated::fire_render_needs"
                );
            }
        }
    }

    #[test]
    fn client_and_world_wrappers_require_their_installed_dispatchers() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        fixture(
            root,
            "sunrise-client-lib/Cargo.toml",
            "[lib]\npath = \"../sunrise-client/src/client_lib.rs\"\n",
        );
        fixture(
            root,
            "sunrise-client/src/client_lib.rs",
            "mod hunger; mod native_client_hooks;",
        );
        fixture(
            root,
            "sunrise-client/src/hunger.rs",
            "fn native_living_heal() { crate::native_client_hooks::living_heal(); }",
        );
        fixture(
            root,
            "sunrise-client/src/native_client_hooks.rs",
            "fn living_heal() { (hooks.living_heal)(); }",
        );
        fixture(root, "sunrise-client/Cargo.toml", "");
        fixture(root, "sunrise-client/src/lib.rs", "mod entry; mod game;");
        fixture(
            root,
            "sunrise-client/src/game.rs",
            "mod heal; pub(crate) use heal::forward as heal_callback;",
        );
        fixture(
            root,
            "sunrise-client/src/game/heal.rs",
            "fn forward() { mod_api::dispatch_living_heal(); }",
        );
        fixture(root, "sunrise-world/Cargo.toml", "");
        fixture(
            root,
            "sunrise-world/src/lib.rs",
            "mod loot; mod native_load_hooks;",
        );
        fixture(
            root,
            "sunrise-world/src/loot.rs",
            "fn native_loot_table_load() { crate::native_load_hooks::loot_table_load(); }",
        );
        fixture(
            root,
            "sunrise-world/src/native_load_hooks.rs",
            "fn loot_table_load() { (hooks.loot_table_load)(); }",
        );
        let mut heal = Hook::sample(
            "sunrise_client_lib::hunger::native_living_heal",
            Side::Server,
        );
        heal.name = "living_heal".into();
        let mut load = Hook::sample("sunrise_world::loot::native_loot_table_load", Side::Server);
        load.name = "loot_table_load".into();
        let hooks = [heal, load];
        for (client, world, bad_hook) in [
            (
                "|| mod_api::dispatch_living_heal()",
                "|| mod_api::dispatch_loot_table_load()",
                None,
            ),
            (
                "game::heal_callback",
                "|| mod_api::dispatch_loot_table_load()",
                None,
            ),
            (
                "|| crate::game::heal_callback()",
                "|| mod_api::dispatch_loot_table_load()",
                None,
            ),
            (
                "|| {}",
                "|| mod_api::dispatch_loot_table_load()",
                Some("living_heal"),
            ),
            (
                "|| mod_api::dispatch_living_heal()",
                "|| {}",
                Some("loot_table_load"),
            ),
        ] {
            fixture(
                root,
                "sunrise-client/src/entry.rs",
                &format!(
                    r"
                use crate::{{game}};
                fn install_client_hooks() {{
                    crate::native_client_hooks::install(crate::native_client_hooks::NativeClientHooks {{ living_heal: {client} }});
                }}
                fn start() {{
                    install_client_hooks();
                    sunrise_world::native_load_hooks::install(sunrise_world::native_load_hooks::NativeLoadHooks {{ loot_table_load: {world} }});
                }}
            "
                ),
            );
            let result = check(root, &hooks);
            if let Some(name) = bad_hook {
                let error = result.unwrap_err();
                assert!(error.starts_with(&format!("{name} (")), "{error}");
                assert!(
                    error.ends_with(&format!("::{name} does not call mod_api::dispatch_{name}")),
                    "{error}"
                );
                assert!(!error.contains('\n'), "{error}");
            } else {
                result.unwrap();
            }
        }
    }

    #[test]
    fn wiring_check_requires_each_shared_effect_callback() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), "sunrise-mod-vocab/Cargo.toml", "");
        fixture(
            root.path(),
            "sunrise-mod-vocab/src/lib.rs",
            "mod parts { mod effects; }",
        );
        const EFFECTS: &str = "sunrise-mod-vocab/src/parts/effects.rs";
        fixture(
            root.path(),
            EFFECTS,
            "fn dispatch_mob_effect_tick() { part.should_apply_effect_tick_this_tick(); }",
        );
        let mut cadence = Hook::sample(crate::catalog::EFFECT_TICK_TARGET, Side::Server);
        cadence.name = "should_apply_effect_tick_this_tick".into();
        let mut apply = cadence.clone();
        apply.name = "apply_effect_tick".into();
        let hooks = [cadence, apply];
        assert_eq!(
            check(root.path(), &hooks).unwrap_err(),
            "apply_effect_tick (mod_api::parts::effects::dispatch_mob_effect_tick): target does not call apply_effect_tick"
        );
        fixture(
            root.path(),
            EFFECTS,
            "fn dispatch_mob_effect_tick() { part.should_apply_effect_tick_this_tick(); part.apply_effect_tick(); }",
        );
        check(root.path(), &hooks).unwrap();
    }
}
