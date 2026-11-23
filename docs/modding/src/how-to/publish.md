# Publishing your mod

Use [the Sunrise mod registry](https://playsunrise.net/mods) to distribute a content or native mod. Both ship as
`.smod` packages. A native release carries its source and its `Cargo.lock`, never compiled libraries: each player's
computer builds a native mod for the exact game it runs.

## How releasing works

Releasing moves to one command, `cargo sunrise-mod release`. It is coming, and this page will describe it once it
ships. With it:

1. You register your mod's repository once at [Submit a mod](https://playsunrise.net/mods/submit).
2. `cargo sunrise-mod release patch` (or `minor`, `major`, or a version) checks the mod, bumps its version, and
   pushes a release tag.
3. The tag runs [the shared release workflow](https://github.com/sunrise-org/sunrise-mods/blob/main/.github/workflows/release.yml),
   which builds the `.smod`, signs it with a GitHub artifact attestation that names your repository, and attaches
   it to a GitHub release. You keep no signing keys.
4. The registry checks the release and opens a registry pull request, which a reviewer approves before the version
   is listed.

The earlier route (`publish`, `keygen`, `sign`, `release plan` and prebuilt binaries) has been removed.
