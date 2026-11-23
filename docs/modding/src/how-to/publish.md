# Publishing your mod

Use [the Sunrise mod registry](https://playsunrise.net/mods) to distribute a content or native mod. Both ship as
`.smod` packages. A native release includes source and its lockfile, not compiled libraries.

## Your mod's page

Edit `page.toml` beside `sunrise-mod.toml`:

- `summary`: a short description of at least 25 characters.
- `description`: a long description of at least 125 characters. You can use Markdown.
- `categories`: an optional list of up to eight IDs from the registry's
  [`categories.toml`](https://github.com/sunrise-org/sunrise-mods/blob/main/registry/categories.toml).
- `[[screenshots]]`: at least one entry, with a `path` such as `page/recipes.png` and an optional `caption`.
  Put the PNG or JPEG images under `page/`. Each can be at most 5 MiB.

Text minimums count Unicode scalar values after trimming surrounding whitespace. Optional `name`, `authors`,
`homepage` and `issues` override the values in `sunrise-mod.toml`. The page uses the mod's icon.

Run `cargo sunrise-mod check` to see what you still need before release. `build` writes the page bundle to
`target/sunrise-mod/<id>.page.tar` separately from the `.smod`, so installing the mod does not download its
screenshots.

## Releasing

Commit your mod's source, `page.toml` and screenshots, and `Cargo.lock` for a native mod. If your repository
doesn't have the Sunrise release workflow yet, run `cargo sunrise-mod release setup` from the mod's folder,
then commit and push the resulting change. Register the repository at
[Submit a mod](https://playsunrise.net/mods/submit) and wait for the registration pull request to merge before
releasing.

From the mod's folder on a clean branch:

```sh
cargo sunrise-mod release patch
```

Use `minor`, `major`, or a higher SemVer instead of `patch` to choose another version. Add `--check` to run the
release checks and preview the changes without touching your checkout. `release` refreshes the workflow caller,
commits the version bump, and pushes the branch and release tag.

The tag runs [the shared release workflow](https://github.com/sunrise-org/sunrise-mods/blob/main/.github/workflows/release.yml).
It builds the `.smod` and page bundle, attests both with GitHub artifact attestations, and attaches the files and
attestation bundles to the GitHub release. The registry bot verifies the release and opens a registry pull
request. The version is listed after that pull request passes its checks and a reviewer approves and merges it.

See the [command reference](../reference/sdk/05-commands.md) for the release and package-check commands.
