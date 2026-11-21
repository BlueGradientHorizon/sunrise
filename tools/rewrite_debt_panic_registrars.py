#!/usr/bin/env python3
"""Rewrite the native registrar crossings for the hook-panic debt item.

Run `ingress` for the concrete owner parameters, then `bridge <hook> ...` once
those hooks opt into the generated staged-surface adapter. Both are repeatable.
"""

import argparse
from pathlib import Path


GAME = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game"
NATIVE = GAME / "native_hooks"
# hook suffix -> (argument, registrar trait, concrete owner)
REGISTRARS = {
    "block_tint_sources": ("sources", "BlockTintSources", "BlockTintRegistry"),
    "color_resolvers": ("resolvers", "ColorResolvers", "ColorResolverRegistry"),
    "cauldron_fluid_content": ("contents", "CauldronFluidContents", "CauldronFluidContentRegistry"),
    "enchantment_damage_effects": ("effects", "EnchantmentDamageEffects", "EnchantmentDamageRegistry"),
    "villager_professions": ("professions", "VillagerProfessions", "ProfessionDeclarations"),
    "entity_renderers": ("renderers", "EntityRenderers", "EntityRendererRegistry"),
    "particle_providers": ("providers", "ParticleProviders", "ParticleProviderRegistry"),
    "special_model_renderer": ("renderers", "SpecialModelRenderers", "SpecialModelRendererRegistry"),
    "game_tests": ("functions", "GameTests", "GameTestFunctions"),
    "load_conditions": ("conditions", "LoadConditions", "LoadConditionDeclarations"),
    "entity_layers": ("layers", "EntityLayers", "LayerRegistration"),
    "client_item_extensions": ("extensions", "mod_api::ClientItemExtensionsRegistrar", "Registration"),
    "cauldron_interaction": ("interactions", "CauldronInteractions", "Registrar"),
}
GAME_OWNERS = {
    "entity_layers": ("modding/native_entity_layers.rs", "native_entity_layers", "pub(super) "),
    "client_item_extensions": ("modding/client_item_extensions.rs", "client_item_extensions", ""),
    "cauldron_interaction": ("blocks/cauldron_interactions.rs", "cauldron_interactions", ""),
}


def replace(source, old, new, path, *formatted, count=1):
    before = source.count(old)
    after = sum(source.count(pattern) for pattern in (new, *formatted))
    if (before, after) == (count, 0):
        return source.replace(old, new)
    if (before, after) == (0, count):
        return source
    raise SystemExit(f"{path}: expected exactly {count} crossing(s): {old!r} or {new!r}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("ingress")
    bridge = commands.add_parser("bridge")
    bridge.add_argument("hooks", nargs="+", choices=REGISTRARS)
    args = parser.parse_args()

    pending = {}
    for suffix in REGISTRARS if args.command == "ingress" else args.hooks:
        argument, trait, owner = REGISTRARS[suffix]
        hook = f"register_{suffix}"
        path = NATIVE / f"{hook}.rs"
        source = path.read_text()
        formatted = ()
        if args.command == "ingress":
            if suffix == "game_tests":
                source = replace(source, "use mod_api::GameTests;\n", "use mod_api::GameTestFunctions;\n", path)
                old = f"fn native_{hook}(&self, {argument}: &mut dyn {trait})"
                new = f"fn native_{hook}(&self, {argument}: &mut {owner})"
            else:
                if suffix == "load_conditions":
                    source = replace(source, "use super::*;\nuse mod_api::LoadConditions;\nimpl", "use super::*;\nimpl", path)
                prefix = f"fn native_{hook}(\n        context: &mod_api::RegistrationContext,\n        "
                old = f"{prefix}{argument}: &mut dyn {trait},"
                if suffix in GAME_OWNERS:
                    owner_file, module, visibility = GAME_OWNERS[suffix]
                    owner_path = GAME / owner_file
                    pending[owner_path] = replace(
                        owner_path.read_text(),
                        f"\n{visibility}struct {owner}<'a>",
                        f"\npub(in crate::game) struct {owner}<'a>",
                        owner_path,
                    )
                    gap = "\n" if suffix == "entity_layers" else ""
                    source = replace(
                        source,
                        f"use super::*;\n{gap}impl Game",
                        f"use super::*;\nuse crate::game::{module}::{owner};\n{gap}impl Game",
                        path,
                    )
                    new = f"{prefix}{argument}: &mut {owner}<'_>,"
                else:
                    new = f"{prefix}{argument}: &mut mod_api::{owner},"
        else:
            prefix = f"self.fire_{hook}(" if suffix == "game_tests" else f"Self::fire_{hook}(context, "
            old = f"{prefix}{argument});"
            new = f"{prefix}|listen| {argument}.register_listener(listen));"
            formatted = (
                f"{prefix}|listen| {{\n            {argument}.register_listener(listen)\n        }});",
            )
        pending[path] = replace(source, old, new, path, *formatted)

    if args.command == "bridge" and "load_conditions" in args.hooks:
        path = GAME / "modding/load_conditions/tests.rs"
        call = "mod_api::dispatch_register_load_conditions(&RegistrationContext::new(), "
        pending[path] = replace(
            path.read_text(),
            call + "&mut registrar);",
            call + "|listen| {\n                registrar.register_listener(listen)\n            });",
            path,
            count=2,
        )

    for path, source in pending.items():
        if source != path.read_text():
            path.write_text(source)


if __name__ == "__main__":
    main()
