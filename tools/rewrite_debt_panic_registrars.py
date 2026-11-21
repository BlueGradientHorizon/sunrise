#!/usr/bin/env python3
"""Rewrite the native registrar crossings for the hook-panic debt item.

Run `ingress` for the concrete owner parameters, then `bridge <hook> ...` once
those hooks opt into the generated staged-surface adapter. Both are repeatable.
"""

import argparse
from pathlib import Path


NATIVE = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game/native_hooks"
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
}


def replace(source, old, new, path, *formatted):
    before = source.count(old)
    after = sum(source.count(pattern) for pattern in (new, *formatted))
    if (before, after) == (1, 0):
        return source.replace(old, new)
    if (before, after) == (0, 1):
        return source
    raise SystemExit(f"{path}: expected exactly one crossing: {old!r} or {new!r}")


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
                new = f"{prefix}{argument}: &mut mod_api::{owner},"
        else:
            prefix = f"self.fire_{hook}(" if suffix == "game_tests" else f"Self::fire_{hook}(context, "
            old = f"{prefix}{argument});"
            new = f"{prefix}|listen| {argument}.register_listener(listen));"
            formatted = (
                f"{prefix}|listen| {{\n            {argument}.register_listener(listen)\n        }});",
            )
        pending[path] = replace(source, old, new, path, *formatted)

    for path, source in pending.items():
        if source != path.read_text():
            path.write_text(source)


if __name__ == "__main__":
    main()
