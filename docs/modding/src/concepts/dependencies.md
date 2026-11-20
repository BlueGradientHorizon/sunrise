# Dependencies between mods

A mod can publish functions and statics from its root crate for another mod to call.
Declare dependencies in `sunrise-mod.toml`, not `Cargo.toml`:

```toml
[dependencies]
base_mod = "^1"
extra_mod = { version = "^2", optional = true }
```

Requirements use Cargo semver syntax and are checked against each installed mod's `sunrise-mod.toml`
`version`, not its Cargo package version. That version is semver, checked when the manifest is read,
so a mod whose version isn't semver never loads, whatever requirement a dependent states. Content-only mods use these declarations to validate their
requirements and, by default, load their layers after dependencies. Native and wasm packages can
depend on content-only packages without linking anything from them. Code libraries link only
within one target. Missing required dependencies, incompatible versions and cycles refuse logical
resolution with the mods named. An absent optional dependency is permitted in the logical graph;
an installed optional dependency must match its requirement.

A standalone build wires only the code projects it is given. Other requirements stay in
`sunrise-mod.toml` and are checked against the installed packages at install and startup.
Code that references an unsupplied library still fails to compile.

The [registry index and installers](../reference/registry-index.md#planning-installs-and-removals) use these
same manifest declarations. A new submission must resolve with the listed dependencies on its Minecraft release.
Optional dependencies never install automatically, but an installed or selected target must match the declared range.
`cargo sunrise-mod check <file>` shows both kinds.

A version such as `3.6.30-sunrise.1` is a SemVer prerelease, even when the mod author uses the suffix to name
an ordinary port release. `^3.6` cannot select it. To require that release, declare `"=3.6.30-sunrise.1"`.

Detailed rows also accept `side`, `order`, `reason` and `url`; see the
[complete manifest schema](../reference/manifest.md). Side filters the declaration by installation
before checking it: an integrated server is client, a dedicated server is server. `order = "before"`
or `"none"` changes listener/content dispatch, not compiler staging or actual imports.

The instance's `sunrise-mods/dependency-overrides.toml` can drop a logical constraint/dispatch edge
or make it a warning, including for a required content package that isn't installed. A removed row
no longer lets a Part attach to that package's effect or block entity type. The policy cannot
remove a native linked library or compiled package requirement, or a wasm pin or import.
Selected content and loaded native/wasm code share one rule resolver; warnings and every applied
override appear on both affected installed Mods rows, alongside any failed/off status.

Native builds add a path dependency for each supplied library using the mod ID as the Cargo alias
and the target's actual Cargo package name. Optional code libraries use a Cargo feature named
exactly as their mod ID and enabled only when that library is supplied. Guard calls with `if_installed!`, which builds the block only with
that feature and runs it only when the mod is installed, so the same source works when built to
`mod.wasm`, where the library is linked at load and an absent one's functions trap:

```rust,ignore
mod_api::if_installed!(host, extra_mod, {
    extra_mod::register_extra(host);
});
```

Do not define these dependencies or features yourself in `Cargo.toml`.
Public statics must use appropriate synchronization for shared and reentrant hooks; private state in
another mod's hook object is not exposed.

Hook listeners run in descending priority, then resolved dependency dispatch order, with dependency-order ties kept in bundle order. Mixin injection order remains
horizon-mixins' priority, mod ID, then injection ID for source splices. Function-slot listeners
instead use descending priority, dependency/registration order, then injection ID within a mod;
call-site listeners retain mod-ID tie ordering.
