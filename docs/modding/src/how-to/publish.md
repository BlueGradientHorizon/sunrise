# Publishing your mod

Sunrise's registry, [sunrise-org/sunrise-mods](https://github.com/sunrise-org/sunrise-mods), lists mods, modpacks,
resource packs and shader packs at [playsunrise.net/mods](https://playsunrise.net/mods). The
[registry submission form](https://playsunrise.net/mods/submit) uploads your package and opens a pull request for a
person to review. To prepare a mod submission locally, follow the steps below with `cargo sunrise-mod publish`,
then upload the package and open the pull request yourself. Native mods for stable releases use the
[stable source submission](#publishing-a-stable-source-version) below.

## 1. Build and write the submission

Set `version` in `sunrise-mod.toml` to a SemVer without `+build` metadata, at most 64 characters after
padding missing parts (for example, `1.2` becomes `1.2.0`). The registry uses that version in the release name.

In your mod's folder:

```sh
cargo sunrise-mod publish --summary "One line about the mod." --owner your-github-login --categories utility
```

It builds the package as `build` does, which may download the SDK, toolchain and dependencies. It does not contact the
registry. The submission goes into `target/sunrise-mod/publish/` (change it with `--out <dir>`):

| Path | What it is |
| --- | --- |
| `mods/<id>.toml` | The registry entry, filled from `sunrise-mod.toml`: name, description, authors, license, `homepage` as the source, and the version's file name, SHA-256 and date |
| `<id>-<version>.smod` | The package: attach exactly this file to a release of your repository |
| `files/<sha256>` | The same file under the name the registry's checks read |
| `icons/<id>.png` | For a new listing, your `icon.png` from beside `sunrise-mod.toml` |
| `PULL_REQUEST.md` | The text of the pull request |

What the manifest cannot say comes from flags: `--summary` (at most 160 characters), `--owner <login>` (repeat it for
several; the GitHub accounts allowed to publish updates), `--categories a,b` (1 to 3 from [your kind's
list](../reference/registry-index.md#submitting-a-project)), `--tags a,b` and `--changelog <text>`. An entry or page
field still missing is written as a value that starts with `TODO(publish)`, and the command lists what remains. The
registry refuses an entry that holds one, so edit `mods/<id>.toml` (or the manifest, then run `publish` again) until the
list is empty.

Save `icon.png` beside `sunrise-mod.toml` as a square PNG, 64 to 512 pixels per side and at most 256 KiB, so it fits
both the package and the registry.

## 2. Check the new file

Run the `cargo sunrise-mod check <file>` command that `publish` prints, from your mod's folder. For example:

```sh
cargo sunrise-mod check 'target/sunrise-mod/publish/my_mod-0.1.0.smod'
```

This checks the new package and, for a signed project, its signature. `publish` also lists anything left to fill in
on the entry or page. Registry CI checks the whole checkout and plans installations after staging, with all listed
files available. Your submission folder does not contain previously published packages or an existing project's images.

For a first listing, you can also try an index containing just your project once the entry and icon are complete:

```sh
cargo sunrise-mod registry build 'target/sunrise-mod/publish' --files 'target/sunrise-mod/publish/files' \
  --out 'target/sunrise-mod/publish/index.json' \
  --release 'https://github.com/sunrise-org/sunrise-mods/releases/download/'
```

## 3. Open the pull request

1. Create a release of your repository and attach `<id>-<version>.smod` to it. Its address is the artifact URL.
2. Fork [sunrise-org/sunrise-mods](https://github.com/sunrise-org/sunrise-mods) and copy in `mods/<id>.toml`.
   For a first listing, copy in `icons/<id>.png` too, and at least one screenshot: save a PNG or JPEG (320×180 to
   3840×2160, at most 1 MiB) as `screenshots/<id>/<name>.png` (or `.jpg`) in your fork and list it in the entry:
   `screenshots = ["screenshots/<id>/<name>.png"]`. A new project without one is refused. A `mod` or `native-mod`
   in the `library` category needs none. Do not commit the package or `files/`.
3. Commit with `git commit -s`: every commit needs a `Signed-off-by:` line.
4. Open the pull request with the text of `PULL_REQUEST.md`, after pasting the artifact URL into it.

## 4. What review and staging do

- A maintainer copies your file from the artifact URL to the registry's `staging` release, named
  `<sha256>.smod`. Nothing is fetched from an address in a pull request on its own, and you cannot stage a file yourself.
- The `check` workflow then downloads each listed file, compares its SHA-256 with the entry's, and runs
  `registry review`, `registry check` and `registry build` on the result.
- A reviewer reads the change and merges it. The `publish` workflow then releases the file as `<id>-<version>` and
  adds the project to the index.
- The hash is checked again before any download reaches the game folder, so a file changed after
  review is refused.

## 5. A new version

Raise `version` in `sunrise-mod.toml`, then run `publish --registry <your checkout of sunrise-mods>` with an output
folder separate from the checkout. For a signed project, [build and sign first](#signing). `publish` adds the new
version row to the entry from the checkout, leaving all existing fields and rows in place. The printed local check
reads only the new file, so you do not need to download the old files or copy over the icon and screenshots.
A new version of a listing that has no screenshots needs one too: add the file under `screenshots/<id>/` and list it
in the entry's `screenshots` in the same pull request, or the check refuses it (a `mod` or `native-mod` in the
`library` category is exempt, and a listing that gains no version stays valid as it is). Repeat steps 2 to 4.

`--registry <checkout>` also checks a first listing's id against the checkout's collection ids, and lists among what
remains each required dependency of the package that no listed mod version for the same game satisfies, with the
versions the checkout does list; a version taken down in `revoked.toml` does not count. These lines are printed only,
not written into the entry. The dependencies are read from the package itself, so after changing `[dependencies]` in
`sunrise-mod.toml`, build again (and sign the new file, for a signed project) before running `publish`. Without
`--registry`, the registry's CI checks both when the pull request opens; it also checks sides and what the dependencies
themselves require.

Published source versions never change. A file that must go is taken down by the registry's maintainers in
`revoked.toml`. The only update `publish` makes to an existing version is an [append of signed native builds](#publishing-native-binaries).

## Signing

Signed projects use an Ed25519 key in [Minisign](https://jedisct1.github.io/minisign/) format. The registry entry
holds the public key; every version of that project must be signed with the same private key.

Run `cargo sunrise-mod keygen` once in your mod project. It reads the mod ID from `sunrise-mod.toml`, asks for a
passphrase without echoing it, and prints the registry public key. It saves the encrypted `signing.key` and a text
copy of the public key (`signing.pub`) in the mod's per-user data folder:

- Linux: `$XDG_DATA_HOME/sunrise-mod/keys/<mod-id>/` (default data home: `~/.local/share`).
- macOS: `~/Library/Application Support/sunrise-mod/keys/<mod-id>/`.
- Windows: `%LOCALAPPDATA%/sunrise-mod/keys/<mod-id>/` (fallback: `%USERPROFILE%/AppData/Local`).

Use `--out <dir>` to choose a different folder, either outside the project or under its top-level `.sunrise/`,
`target/`, or `.git/` directory, which are excluded from packages. The tool does not change Git's ignore rules.
Back up the private key securely; never commit it or upload it as an asset.

Build and sign before calling `publish`, including on the first listing. For example, for a mod with ID `my_mod`,
set `public_key` to the value printed by `keygen`:

```sh
cargo sunrise-mod build
cargo sunrise-mod sign target/sunrise-mod/my_mod.smod
cargo sunrise-mod publish --summary "..." --owner you --categories utility \
  --key "$public_key" --signature target/sunrise-mod/my_mod.smod.minisig
```

`sign` writes a detached `.minisig` beside the input. It uses the current mod's per-user `signing.key` unless
`--key <path>` is given. Existing Minisign key files and signatures work too. `keygen --no-password` is an explicit opt-in
to an unencrypted private key; with a local file, use `sign --no-password` for such a key. Neither command
overwrites an existing output. If you change the package, build it and sign the new bytes rather than reusing an old
signature.

For CI, store the full private key file text as a protected secret named `SUNRISE_MOD_SIGNING_KEY` and, for an
encrypted key, store its passphrase separately as `SUNRISE_MOD_SIGNING_PASSPHRASE`. Set those only in the
signing step's environment and run `cargo sunrise-mod sign <file> --key-env`. With a passwordless CI key, leave
the passphrase variable unset. The command never prompts in this mode. Do not put either secret on the command
line, in a log, or in a build artifact.

For a new version of an already-signed project, use the same key and replace the page flags (`--summary`, `--owner`,
`--categories`) with `--registry <your checkout of sunrise-mods>`. With `--key` and `--signature`, `publish` checks
and copies the exact bytes you signed, without rebuilding.

## Publishing a stable source version

You can publish the signed `.smod` without binaries, including for a mod that is not eligible to ship them.
Set `homepage` in the manifest to your project's GitHub repository. Pin the project to the stable Sunrise
commit, build and [sign the source package](#signing), and pass a release list (`releases.json`, an array of
`{ "tag", "commit" }` records). Use `--asset-base` for your GitHub release's download URL without the asset name:

```sh
cargo sunrise-mod publish --summary 'A brief summary' --owner you --categories utility \
  --sunrise 26.3-0.1.0 --releases releases.json \
  --key "$public_key" --signature target/sunrise-mod/my_mod.smod.minisig \
  --package target/sunrise-mod/my_mod.smod \
  --asset-base 'https://github.com/you/my_mod/releases/download/v0.1.0'
```

Upload only the author-release assets listed in `PULL_REQUEST.md`: the exact `.smod` and `.smod.minisig`.
Submit `mods/<id>.toml` to the registry with `icons/<id>.png` if an icon was staged. The `file` field already
points to your release asset. Registry CI downloads it from there, verifies it and mirrors it after merge.
For a new version of an existing listing, use `--registry <checkout>` instead of the page flags.

## Publishing native binaries

Builds are optional. They are available only to a standalone API native mod: no game patches, crates.io dependencies
or declared mod dependencies. Keep the signed `.smod` as the source release even when players can download binaries.
Set the manifest's `homepage` to your project's GitHub repository. Use the stable release list (`releases.json`,
an array of `{ "tag", "commit" }` records). To publish binaries, put the verified SDK index (`sdk.json`),
`surface.json`, and extracted `build-data/` under `sdk-data/<commit>/` for both the original source package's
pin and the target commit. To describe the archives with `--describe-binaries`, only the target's verified
`sdk.json` is needed under `sdk-data/`.

1. Pin your checkout to the stable SDK commit. On each SDK host, build the archive with:

   ```sh
   cargo sunrise-mod build --binaries release-assets --sunrise-release 26.3-0.1.0 --releases releases.json
   ```

   Run it with `--server` as well for each dedicated-server host the SDK lists. Every run writes a `.smod` and the
   selected host's `<id>-<version>+<release>-<host>.tar.xz`. Keep one `.smod` to sign as the source for a new version.
2. Sign every archive with `cargo sunrise-mod sign <archive>`. Gather the archives and their `.minisig` files
   in `release-assets`, then assemble and sign the descriptor with the same key:

   ```sh
   cargo sunrise-mod build --describe-binaries release-assets --sunrise-release 26.3-0.1.0 \
     --releases releases.json --sdk-data sdk-data
   cargo sunrise-mod sign release-assets/my_mod-0.1.0+26.3-0.1.0.binaries.json
   ```

   The descriptor embeds the archive signatures; it does not sign the archives for you.
3. Prepare the submission. For a new listing:

   ```sh
   cargo sunrise-mod sign target/sunrise-mod/my_mod.smod
   cargo sunrise-mod publish --summary 'A brief summary' --owner you --categories utility \
     --sunrise 26.3-0.1.0 --releases releases.json --sdk-data sdk-data \
     --key "$public_key" --signature target/sunrise-mod/my_mod.smod.minisig \
     --package target/sunrise-mod/my_mod.smod \
     --binaries release-assets --asset-base 'https://github.com/you/my_mod/releases/download/v0.1.0'
   ```

   For an existing listing with a new mod version, replace the page flags with `--registry <checkout>`. For a later
   Sunrise build of an **already published** mod version, build archives from a temporary checkout of that source
   repinned to the new commit. Use `--registry <checkout>` and pass the **original published `.smod`** to `--package`,
   not the temporary build's package. Omit `--sunrise`, `--key`, and `--signature`: `publish` uses the source row's
   existing stable pin, key and signature and adds one build row without touching them. Keep `--asset-base`
   on the original author release. `publish` checks the signed assets and the registry's fit verdict before writing output.
   Use the output only after `publish` succeeds.
4. Upload and verify only the author-release assets listed in `PULL_REQUEST.md` on the release at `--asset-base`
   before opening the pull request. These are the archives, the descriptor and its `.binaries.json.minisig`, and,
   for a new mod version only, the source package and its `.smod.minisig`. The descriptor embeds the archive
   signatures, so no separate archive signature uploads are needed. If an upload fails partway through, upload
   the missing files from the same verified output rather than rebuilding or re-signing. The tool does not upload anything.
   Submit `mods/<id>.toml` with `icons/<id>.png` if an icon was staged; do not commit the release assets. See the
   [registry format](../reference/registry-index.md#stable-native-builds) for offline verification and takedowns.

## A collection

Add `collections/<id>.toml` and a square PNG at `icons/<id>.png` in one pull request to
[sunrise-org/sunrise-mods](https://github.com/sunrise-org/sunrise-mods). A collection is not a package: there's no file
upload or version to publish.

```toml
id = "sunrise_creatures"
name = "Sunrise Creatures"
summary = "Fireflies, butterflies and their shared library."
owners = ["octocat"]
icon = "icons/sunrise_creatures.png"
members = [
  { id = "creature" },
  { id = "fireflies" },
  { id = "butterflies", default = false },
]
```

Members are selected by default unless you set `default = false`. You can add a Markdown `description` and record
numeric `owner_ids` in the same order as `owners`. `cargo sunrise-mod registry check <base> <head> --files <dir>`
checks the proposed registry, including the collection; see the [full rules](../reference/registry-index.md#collections).

To add a new mod, list it in the registry, then add its id to `members` in one collection pull request. If you own both
the collection and the mod, this is a routine membership edit; changes to the page, defaults or owners wait for review.

## Integrity and takedowns

- **Integrity.** Every download is checked against the SHA-256 the index lists before the file reaches the game
  folder. Unsigned local files stay allowed, with an "unsigned" mark.
- **Takedowns.** A file the registry takes down (`revoked.toml`) is refused and turned off where installed. What a
  listed project agrees to is in the registry's `MOD-POLICY.md`.

Once a project is published, players can rate it on its page after signing in with GitHub. A player can change their
rating later. Owners can't rate their own project, and nobody sees who rated.
