#!/usr/bin/env python3
"""Run the generated native release steps offline on the daily run's signed Linux SDK."""

import argparse
import os
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import tomllib
import zlib


ROOT = Path(__file__).resolve().parents[2]


def icon(path):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    size = 128
    header = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    pixels = (b"\0" + b"\x76\x91\xab" * size) * size
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))


def linux_sdk(batch, base, work, linux, release):
    # A daily rerun may use an already-published SDK with more than the Linux legs. Project the
    # already authenticated index onto the two Linux hosts in a debug-only signed scratch copy.
    original = base / release
    dist = work / "linux-sdk" / release
    dist.mkdir(parents=True)
    index = release_api.json_read(batch.sdk / batch.plan["target"]["commit"] / "sdk.json")
    index["assets"] = {name: asset for name, asset in index["assets"].items() if asset.get("target", linux) == linux}
    for name in index["assets"]:
        (dist / name).hardlink_to(original / name)
    release_api.json_write(dist / "sdk.json", index)
    release_api.call("bash", ROOT / "tools/ci/sign_release.sh", release, dist / "sdk.json",
                     cwd=ROOT, env={**os.environ, "EPHEMERAL": "yes"})
    os.environ["SUNRISE_SDK_TRUSTED_KEY"] = (dist / "ephemeral.pub").read_text().strip()
    os.environ["SUNRISE_SDK_BASE"] = str(dist.parent)


def entry(from_stage, to_checkout, id):
    for directory, name in [("mods", f"{id}.toml"), ("icons", f"{id}.png")]:
        if (from_stage / directory / name).is_file():
            (to_checkout / directory).mkdir(parents=True, exist_ok=True)
            shutil.copyfile(from_stage / directory / name, to_checkout / directory / name)
    return tomllib.loads((to_checkout / "mods" / f"{id}.toml").read_text(encoding="utf-8"))


def exercise(work, project, commit):
    repo, tag = "example/sunrise_release_probe", "v0.1.0"
    manifest = project / "sunrise-mod.toml"
    page = (f'homepage = "https://github.com/{repo}"\nlicense = "MIT"\n'
            'description = "A disposable native sample that checks release builds for both Linux sides."')
    manifest.write_text(manifest.read_text(encoding="utf-8").replace('description = ""', page), encoding="utf-8")
    icon(project / "icon.png")
    sdk_base = Path(os.environ["SUNRISE_SDK_BASE"])
    game = release_api.json_read(sdk_base / f"sdk-{commit}" / "sdk.json")["build"]["game"]
    stable = f"{game}-0.1.0"
    releases = work / "stable.json"
    release_api.json_write(releases, [{"tag": stable, "commit": commit}])
    release_work = work / "release"
    release_api.plan(release_work, project, commit, repo, tag, releases)
    batch = release_api.Batch(release_work)
    linux = release_api.LINUX
    wanted_hosts = {linux, f"{linux}-server"}
    if set(batch.hosts) != wanted_hosts:
        linux_sdk(batch, sdk_base, work, linux, f"sdk-{commit}")
        release_work = work / "linux-release"
        release_api.plan(release_work, project, commit, repo, tag, releases)
        batch = release_api.Batch(release_work)
    release_api.require(set(batch.hosts) == wanted_hosts, "this smoke needs both Linux SDK sides")
    for host in batch.hosts:
        release_api.build(batch, project, host)

    private = work / "author-key"
    key = release_api.tool("keygen", "--out", private, "--no-password", cwd=project, capture=True).strip()
    os.environ[release_api.KEY] = (private / "signing.key").read_text(encoding="utf-8")
    os.environ[release_api.PASSPHRASE] = ""  # Actions supplies an empty value for an unset secret.
    try:
        release_api.sign(batch, project)
    finally:
        os.environ.pop(release_api.KEY)
        os.environ.pop(release_api.PASSPHRASE)
        shutil.rmtree(private)
    os.environ.update(SUNRISE_MOD_PUBLIC_KEY=key, SUNRISE_MOD_REGISTRY_OWNER="example",
                      SUNRISE_MOD_SUMMARY="Native release smoke.", SUNRISE_MOD_CATEGORIES="library")

    empty = work / "empty"
    (empty / "mods").mkdir(parents=True)
    first = work / "first-listing"
    release_api.stage(batch, project, empty, first)
    seeded = work / "source-only"
    release_api.stage(batch, project, empty, seeded, source_only=True)
    before = work / "base"
    held = entry(seeded, before, batch.id)
    append = work / "append"
    release_api.tool("publish", "--out", append, "--registry", before, "--package", batch.assets / batch.source,
                     "--binaries", batch.assets, "--sdk-data", batch.sdk, "--releases", batch.releases,
                     "--asset-base", f"https://github.com/{repo}/releases/download/{tag}", cwd=project)
    shutil.copytree(seeded / "files", append / "files", dirs_exist_ok=True)
    for staged in [first, append]:
        checkout = staged / "registry"
        if staged == append:
            shutil.copytree(before, checkout)
        updated = entry(staged, checkout, batch.id)
        row = updated["versions"][0]
        builds = row.pop("builds")
        assert row == held["versions"][0], "the reviewed source row changed"
        assert len(builds) == 1 and builds[0]["sunrise"] == stable, "expected one appended build"
        assert updated["key"] == held["key"] == key
        index = staged / "index.json"
        release_api.tool("registry", "build", checkout, "--files", staged / "files", "--out", index,
                         "--releases", batch.releases, "--sdk-data", batch.sdk)
        published = release_api.json_read(index)["projects"][0]["versions"][0]["native_binaries"]
        assert {(item["build"], item["target"]) for item in published} == {(commit, host) for host in wanted_hosts}
    print("Generated release: Linux client and server, signed first listing and unchanged-source append passed.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    os.environ["SUNRISE_MOD_TOOL"] = str(args.tool.resolve())
    # Exercise the scripts embedded in the same tool that generated the workflow YAML.
    with tempfile.TemporaryDirectory(dir=args.scratch) as scratch:
        work = Path(scratch)
        os.environ["SUNRISE_MOD_CACHE"] = str(work / "cache")
        project = work / "sunrise_release_probe"
        import subprocess
        subprocess.run([os.environ["SUNRISE_MOD_TOOL"], "new", project, "--template", "native", "--sunrise", args.commit], check=True)
        scripts = work / "scripts"
        subprocess.run([os.environ["SUNRISE_MOD_TOOL"], "release", "scripts", "--out", scripts], check=True)
        sys.dont_write_bytecode = True
        sys.path.insert(0, str(scripts))
        global release_api
        import sunrise_release as release_api
        exercise(work, project, args.commit)


if __name__ == "__main__":
    main()
