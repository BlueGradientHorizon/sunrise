//! The source-only CI check for catalog targets. Build generation never calls the check.
use crate::{Catalog, Hook, catalog::DispatchStyle, dispatch, free_function_source, source};
use mod_source::modules::test_only;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use syn::{Block, Expr, Item, visit::Visit};

/// Check that each declared hook target reaches its adapter or dispatcher. Never writes sources.
pub fn check_hook_wiring(root: &Path) -> Result<(), String> {
    check(root, &Catalog::load(root)?.hooks)
}

pub(super) fn check(root: &Path, hooks: &[Hook]) -> Result<(), String> {
    let mut files = BTreeMap::new();
    let errors: Vec<_> = hooks
        .iter()
        .filter_map(|hook| {
            check_target(root, &mut files, hook)
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

const NATIVE_HOOKS: &str = "sunrise_client::game::native_hooks::";

fn check_target(
    root: &Path,
    files: &mut BTreeMap<PathBuf, syn::File>,
    hook: &Hook,
) -> Result<(), String> {
    let (krate, module, owner, target) = if let Some(path) = hook.target.strip_prefix(NATIVE_HOOKS)
    {
        let (module_and_owner, target) = path.rsplit_once("::").ok_or("expected a method")?;
        let (module, owner) = module_and_owner
            .rsplit_once("::")
            .ok_or("expected a method owner")?;
        (
            "sunrise-client",
            format!("game::native_hooks::{module}"),
            Some(owner),
            target.to_owned(),
        )
    } else {
        let (krate, path) = free_function_source(&hook.target).ok_or("unknown target crate")?;
        let (module, target) = path.rsplit_once("::").ok_or("expected a free function")?;
        (krate, module.to_owned(), None, target.to_owned())
    };
    let module = root
        .join("crates")
        .join(krate)
        .join("src")
        .join(module.replace("::", "/"));
    let flat = module.with_extension("rs");
    let path = if flat.is_file() {
        flat
    } else {
        module.join("mod.rs")
    };
    let file = match files.entry(path) {
        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::btree_map::Entry::Vacant(entry) => {
            let path = entry.key();
            let read_error = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
            let text = fs::read_to_string(path).map_err(|error| read_error(&error))?;
            let file = syn::parse_file(&text).map_err(|error| read_error(&error))?;
            entry.insert(file)
        }
    };
    let bodies: Vec<&Block> = if let Some(owner) = owner {
        file.items
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
        file.items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(function)
                    if function.sig.ident == target && !test_only(&function.attrs) =>
                {
                    Some(function.block.as_ref())
                }
                _ => None,
            })
            .collect()
    };
    if test_only(&file.attrs) || bodies.is_empty() {
        return Err("target definition not found".into());
    }
    let name = &hook.name;
    if dispatch::is_render(hook) {
        // These raw functions are intentionally empty replacement slots. Their generated
        // adapter calls them, so look for the other edge: the renderer calling the adapter.
        let fire = format!("generated::fire_{name}");
        if !file.items.iter().any(|item| match item {
            Item::Fn(function) if !test_only(&function.attrs) => {
                calls(&function.block, None, &[&fire])
            }
            _ => false,
        }) {
            return Err(format!("render module does not call {fire}"));
        }
    } else {
        let fire = format!("fire_{name}");
        let associated = format!("Self::{fire}");
        let direct = format!("mod_api::dispatch_{name}");
        let internal = format!("crate::dispatch_{name}");
        let client = format!("crate::native_client_hooks::{name}");
        let load = format!("crate::native_load_hooks::{name}");
        let (method, paths): (Option<&str>, Vec<&str>) =
            if hook.target == crate::catalog::EFFECT_TICK_TARGET {
                // The shared effect dispatcher invokes each Part method itself.
                (Some(name), vec![])
            } else if owner.is_some() && hook.dispatch != DispatchStyle::Direct {
                (Some(&fire), vec![&associated])
            } else if krate == "sunrise-mod-vocab" {
                (None, vec![&internal])
            } else if hook.target.starts_with("sunrise_client_lib::") {
                (None, vec![&direct, &client])
            } else if krate == "sunrise-world" {
                (None, vec![&load])
            } else {
                (None, vec![&direct])
            };
        if bodies.iter().any(|body| !calls(body, method, &paths)) {
            let expected = method
                .into_iter()
                .chain(paths)
                .collect::<Vec<_>>()
                .join(" or ");
            return Err(format!("target does not call {expected}"));
        }
    }
    Ok(())
}

fn calls(block: &Block, method: Option<&str>, paths: &[&str]) -> bool {
    has_call_matching(block, |call| match call {
        Callee::Method(called) => method.is_some_and(|method| called == method),
        Callee::Function(called) => paths.iter().any(|path| {
            let mut names = path.split("::");
            called
                .segments
                .iter()
                .all(|segment| names.next().is_some_and(|name| segment.ident == name))
                && names.next().is_none()
        }),
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
    struct Calls<F> {
        matches: F,
        found: bool,
    }
    impl<'ast, F: Fn(Callee<'ast>) -> bool> Visit<'ast> for Calls<F> {
        // An unrelated local function's body does not wire its enclosing hook.
        fn visit_item(&mut self, _: &'ast Item) {}

        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            self.found |= (self.matches)(Callee::Method(&call.method));
            syn::visit::visit_expr_method_call(self, call);
        }

        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let Expr::Path(call) = &*call.func
                && call.qself.is_none()
            {
                self.found |= (self.matches)(Callee::Function(&call.path));
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    let mut check = Calls {
        matches,
        found: false,
    };
    check.visit_block(block);
    check.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Side;

    #[test]
    fn wiring_check_requires_a_call_in_the_declared_target() {
        let root = tempfile::tempdir().unwrap();
        let source = root
            .path()
            .join("crates/sunrise-client/src/game/native_hooks/pickup.rs");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        let mut hook = Hook::sample(
            "sunrise_client::game::native_hooks::pickup::Game<'_>::native_item_picked_up",
            Side::Server,
        );
        hook.name = "item_picked_up".into();
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
            fs::write(&source, &text).unwrap();
            hook.dispatch = dispatch;
            let result = check(root.path(), std::slice::from_ref(&hook));
            assert_eq!(result.is_ok(), expected, "{body}: {result:?}");
            assert_eq!(fs::read_to_string(&source).unwrap(), text);
        }
    }

    #[test]
    fn wiring_check_checks_render_callers_and_each_shared_effect_callback() {
        let root = tempfile::tempdir().unwrap();
        let render = root.path().join("crates/sunrise-mod-render/src/render.rs");
        let effects = root
            .path()
            .join("crates/sunrise-mod-vocab/src/parts/effects.rs");
        for path in [&render, &effects] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        fs::write(
            &render,
            "fn world_drawn() {} fn draw() { generated::fire_world_drawn(); }",
        )
        .unwrap();
        fs::write(
            &effects,
            "fn dispatch_mob_effect_tick() { part.should_apply_effect_tick_this_tick(); }",
        )
        .unwrap();
        let mut world_drawn = Hook::sample("mod_render::render::world_drawn", Side::Client);
        world_drawn.name = "world_drawn".into();
        let mut cadence = Hook::sample(crate::catalog::EFFECT_TICK_TARGET, Side::Server);
        cadence.name = "should_apply_effect_tick_this_tick".into();
        let mut apply = cadence.clone();
        apply.name = "apply_effect_tick".into();
        let hooks = [world_drawn, cadence, apply];
        assert_eq!(
            check(root.path(), &hooks).unwrap_err(),
            "apply_effect_tick (mod_api::parts::effects::dispatch_mob_effect_tick): target does not call apply_effect_tick"
        );
        fs::write(
            &effects,
            "fn dispatch_mob_effect_tick() { part.should_apply_effect_tick_this_tick(); part.apply_effect_tick(); }",
        )
        .unwrap();
        fs::write(
            &render,
            "fn world_drawn() {} #[cfg(test)] fn not_a_runtime_caller() { generated::fire_world_drawn(); }",
        )
        .unwrap();
        assert_eq!(
            check(root.path(), &hooks).unwrap_err(),
            "world_drawn (mod_render::render::world_drawn): render module does not call generated::fire_world_drawn"
        );
    }
}
