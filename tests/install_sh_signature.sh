#!/bin/bash
# PURPOSE: proves packaging/server/install.sh checks the release signature (docs/release-flow.md, "Signed checksums").
# WHY: the first install used to trust only the SHA-256 GitHub publishes (docs/security.md).
# REFS: tools/ci/sign_release.sh (signs the fake releases with a throwaway key), crates/sunrise-release-sign.
# LOGS: prints one ok/FAIL line per case; exit 1 when any case fails.
# READS-BY: tests/test_install_sh_signature.py (unittest wrapper), by hand: bash tests/install_sh_signature.sh
# ENDS-WHEN: it ends by itself; leaves nothing outside its own mktemp folder.
# Needs OpenSSL 3 (to sign the throwaway release); skips (exit 77) without it. curl is replaced by a fake that
# serves local files, so nothing touches the network.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
openssl version | grep -Eq '^OpenSSL [3-9]\.' || { echo "skip: OpenSSL 3 needed"; exit 77; }
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
fails=0
check() {  # name, expected exit (0 or 1), output grep (may be empty), then the case's exit and output files
  local name=$1 want=$2 pat=$3 rc=$4 out=$5
  local exit_ok=no
  if [ "$want" = 0 ]; then [ "$rc" = 0 ] && exit_ok=yes; else [ "$rc" != 0 ] && exit_ok=yes; fi
  if [ "$exit_ok" = yes ] && { [ -z "$pat" ] || grep -q -- "$pat" "$out"; }; then
    echo "ok   $name"
  else echo "FAIL $name (exit $rc, wanted $want, pattern '$pat')"; sed 's/^/     /' "$out"; fails=$((fails+1)); fi
}

# A fake release: an archive holding sunrise-server, signed for TAG with a throwaway key. A Nightly (TAG nightly)
# is also signed for its own version ALSO (nightly-<date>-<commit>) in SHA256SUMS.v2.sig, titled as
# nightly-build.yml titles it.
make_release() {  # dir tag [also]
  local d=$1 tag=$2 also=${3:-}
  mkdir -p "$d/src/sunrise-server-x" "$d/files"
  printf '#!/bin/sh\necho fake\n' > "$d/src/sunrise-server-x/sunrise-server"; chmod +x "$d/src/sunrise-server-x/sunrise-server"
  tar -czf "$d/files/sunrise-server-linux-x86_64.tar.gz" -C "$d/src" sunrise-server-x
  ALSO_FOR=$also EPHEMERAL=yes bash "$root/tools/ci/sign_release.sh" "$tag" "$d/files/sunrise-server-linux-x86_64.tar.gz" > /dev/null
  json "$d" "$tag" yes "$(title "$tag" "$also")"
}
title() {  # tag also: the release's GitHub title
  if [ -n "$2" ]; then printf 'Nightly %s · %s' "$(printf '%s' "$2" | cut -c9-18)" "$(printf '%s' "$2" | cut -c20-)"
  else printf '%s' "$1"; fi
}
json() {  # dir tag with-sums [title]: GitHub's release JSON, as install.sh reads it
  local d=$1 tag=$2 sums=$3 name=${4:-$2} sha
  sha=$(sha256sum "$d/files/sunrise-server-linux-x86_64.tar.gz" | cut -d' ' -f1)
  {
    printf '{\n  "tag_name": "%s",\n  "name": "%s",\n  "draft": false,\n  "prerelease": false,\n  "assets": [\n' "$tag" "$name"
    printf '    {\n      "name": "sunrise-server-linux-x86_64.tar.gz",\n      "digest": "sha256:%s",\n      "browser_download_url": "https://dl.test/sunrise-server-linux-x86_64.tar.gz"\n    }' "$sha"
    if [ "$sums" = yes ]; then for f in SHA256SUMS SHA256SUMS.sig SHA256SUMS.v2.sig; do
      [ -f "$d/files/$f" ] || continue
      printf ',\n    {\n      "name": "%s",\n      "browser_download_url": "https://dl.test/%s"\n    }' "$f" "$f"; done; fi
    printf '\n  ]\n}\n'
  } > "$d/release.json"
}
# Tools install.sh may use, without openssl (the "no OpenSSL" case).
mkdir "$W/nossl"
for t in sh bash cat awk sed grep cut head find mkdir mktemp rm mv ln chmod cp basename dirname tar gzip sha256sum ls uname printf; do
  p=$(command -v "$t") && ln -s "$p" "$W/nossl/$t"
done
# The fake curl: the API address answers with release.json, any other address with the file of that name.
mkdir "$W/fake"
cat > "$W/fake/curl" <<'CURL'
#!/bin/sh
out=; url=
while [ $# -gt 0 ]; do case "$1" in -o) out=$2; shift 2 ;; -*) shift ;; *) url=$1; shift ;; esac; done
case "$url" in
  # The release list, page by page (page-<n>.json); a page with no file fails as GitHub's error would.
  *"/releases?per_page=100&page="*) src=$REL/page-${url##*page=}.json ;;
  # One release by tag: tag-<tag>.json when the case has one, else release.json.
  https://api.github.com/*/releases/tags/*)
    src=$REL/tag-${url##*/tags/}.json; [ -f "$src" ] || src=$REL/release.json ;;
  https://api.github.com/*) src=$REL/release.json ;;
  *) src=$REL/files/$(basename "$url") ;;
esac
[ -f "$src" ] || exit 22
if [ -n "$out" ]; then cp "$src" "$out"; else cat "$src"; fi
CURL
chmod +x "$W/fake/curl"
# The fake uname: install.sh picks the Linux x86-64 archive, the one these releases hold, on any machine.
cat > "$W/fake/uname" <<'UNAME'
#!/bin/sh
case "$1" in -m) echo x86_64 ;; *) echo Linux ;; esac
UNAME
chmod +x "$W/fake/uname"

# install.sh with the throwaway public key in place of the release key.
with_key() {  # dir -> prints path of the patched copy
  local d=$1
  sed "s|^RELEASE_KEYS=.*|RELEASE_KEYS=\"$(cat "$d/files/ephemeral.pub")\"|" "$root/packaging/server/install.sh" > "$d/install.sh"
  grep -q "$(cat "$d/files/ephemeral.pub")" "$d/install.sh" || { echo "FAIL could not set the test key"; exit 1; }
}
run() {  # dir, path-dir, ...install args: runs the patched install.sh with fake curl
  local d=$1 p=$2; shift 2
  REL=$d PATH="$W/fake:$p" HOME="$W/home" sh "$d/install.sh" --dir "$d/inst" --bin "$d/bin" "$@" > "$d/out" 2>&1
  echo $? > "$d/rc"
}
case_run() { run "$@"; }
verdict() { check "$1" "$2" "$3" "$(cat "$4/rc")" "$4/out"; }
# sed -i differs between GNU and BSD (macOS) sed: edit through a copy instead.
edit() {  # sed-expression file
  sed "$1" "$2" > "$2.tmp" && mv "$2.tmp" "$2"
}
mkdir -p "$W/home"
# The cases get the OpenSSL 3 checked above, wherever it is: macOS's /usr/bin has LibreSSL.
mkdir "$W/ssl"; ln -s "$(command -v openssl)" "$W/ssl/openssl"
P=$W/ssl:/usr/bin:/bin

# 1. Good signature, a Nightly: SHA256SUMS.v2.sig signed for the version its title names.
NIGHTLY=nightly-2026-11-03-0c02067
d=$W/good; make_release "$d" nightly "$NIGHTLY"; with_key "$d"
run "$d" "$P"; verdict "good signature installs (Nightly, signed for its own version)" 0 "Release signature OK" "$d"
[ -x "$d/inst/current/sunrise-server" ] || { echo "FAIL nothing installed"; fails=$((fails+1)); }
grep -q "Downloading Sunrise $NIGHTLY " "$d/out" || { echo "FAIL not installed as $NIGHTLY"; fails=$((fails+1)); }

# 1b. A Nightly with only the channel's signature (published before v2): refused, never the channel's signature.
d=$W/channel; make_release "$d" nightly; with_key "$d"; json "$d" nightly yes "$(title nightly "$NIGHTLY")"
run "$d" "$P"; verdict "Nightly without SHA256SUMS.v2.sig refused" 1 "has no SHA256SUMS.v2.sig" "$d"
[ ! -e "$d/inst/current" ] || { echo "FAIL a Nightly without v2 was installed"; fails=$((fails+1)); }

# 1c. An older Nightly's files and signature retitled as a newer one: its v2 signature names the old version.
d=$W/rollback; make_release "$d" nightly nightly-2026-11-01-1111111; with_key "$d"
json "$d" nightly yes "$(title nightly "$NIGHTLY")"
run "$d" "$P"; verdict "older Nightly passed off as a newer one refused" 1 "signature doesn't match" "$d"

# 1d. A Nightly whose title doesn't name its date and commit: refused, not dated some other way.
d=$W/title; make_release "$d" nightly "$NIGHTLY"; with_key "$d"; json "$d" nightly yes "Nightly"
run "$d" "$P"; verdict "Nightly with an unreadable title refused" 1 "title doesn't name its date and commit" "$d"

# 2. Good signature, a normal tag.
d=$W/tag; make_release "$d" v1.2.3; with_key "$d"
run "$d" "$P" --version v1.2.3; verdict "good signature installs (release tag)" 0 "Release signature OK" "$d"

# 3. Tampered archive: GitHub's digest is made to agree with the swapped archive, only SHA256SUMS catches it.
d=$W/arc; make_release "$d" v1.2.3; with_key "$d"
printf 'x' >> "$d/files/sunrise-server-linux-x86_64.tar.gz"; json "$d" v1.2.3 yes
run "$d" "$P" --version v1.2.3; verdict "tampered archive refused" 1 "doesn't match its SHA-256" "$d"
[ ! -e "$d/inst/current" ] || { echo "FAIL tampered archive was installed"; fails=$((fails+1)); }

# 4. Tampered SHA256SUMS (the archive's line swapped for the hash of the tampered archive).
d=$W/sums; make_release "$d" v1.2.3; with_key "$d"
printf 'x' >> "$d/files/sunrise-server-linux-x86_64.tar.gz"
new=$(sha256sum "$d/files/sunrise-server-linux-x86_64.tar.gz" | cut -d' ' -f1)
printf '%s  sunrise-server-linux-x86_64.tar.gz\n' "$new" > "$d/files/SHA256SUMS"; json "$d" v1.2.3 yes
run "$d" "$P" --version v1.2.3; verdict "tampered SHA256SUMS refused" 1 "signature doesn't match" "$d"

# 5. A signature made for another tag does not pass (a release's list replayed under another tag).
d=$W/replay; make_release "$d" v1.2.3; with_key "$d"
edit 's/"tag_name": "v1.2.3"/"tag_name": "v9.9.9"/' "$d/release.json"
run "$d" "$P" --version v9.9.9; verdict "signature replayed under another tag refused" 1 "signature doesn't match" "$d"

# 6. Signed by a key that is not the release key.
d=$W/wrong; make_release "$d" v1.2.3; with_key "$d"
edit 's|^RELEASE_KEYS=.*|RELEASE_KEYS="r53xb3cuafGxZSicLz06KC+xXp9NJdPH8snNmXo5pAc="|' "$d/install.sh"
run "$d" "$P" --version v1.2.3; verdict "wrong key refused" 1 "signature doesn't match" "$d"

# 7. No OpenSSL: says so, keeps the SHA-256 check.
d=$W/none; make_release "$d" v1.2.3; with_key "$d"
run "$d" "$W/nossl" --version v1.2.3; verdict "no OpenSSL: installs, says it was not checked" 0 "OpenSSL isn't installed" "$d"
printf 'x' >> "$d/files/sunrise-server-linux-x86_64.tar.gz"
d2=$W/none2; cp -r "$d" "$d2"; rm -rf "$d2/inst" "$d2/bin"; edit 's/"digest": "sha256:[0-9a-f]*"/"digest": "sha256:0000"/' "$d2/release.json"
run "$d2" "$W/nossl" --version v1.2.3; verdict "no OpenSSL: a wrong GitHub SHA-256 still refused" 1 "doesn't match its SHA-256" "$d2"

# 8. Old OpenSSL and LibreSSL (macOS): says so, keeps the SHA-256 check.
for v in "OpenSSL 1.1.1w  11 Sep 2023" "LibreSSL 3.3.6"; do
  d=$W/old; rm -rf "$d"; make_release "$d" v1.2.3; with_key "$d"
  mkdir -p "$W/old-bin"; rm -f "$W/old-bin/openssl"
  printf '#!/bin/sh\necho "%s"\n' "$v" > "$W/old-bin/openssl"; chmod +x "$W/old-bin/openssl"
  REL=$d PATH="$W/old-bin:$W/fake:$W/nossl" HOME="$W/home" sh "$d/install.sh" --dir "$d/inst" --bin "$d/bin" --version v1.2.3 > "$d/out" 2>&1; echo $? > "$d/rc"
  verdict "${v%% *}: installs, says it can't check signatures" 0 "can't check Ed25519" "$d"
done

# 9. A release published before signing (no SHA256SUMS): says so, keeps the SHA-256 check.
d=$W/legacy; make_release "$d" v1.2.3; with_key "$d"; json "$d" v1.2.3 no
run "$d" "$P" --version v1.2.3; verdict "unsigned release: installs, says so" 0 "no signed checksums" "$d"

# 10. --version stable: the newest published stable release from every page, numbers compared as numbers
# (0.1.1000000000 is newer than 0.1.999999999, which a padded-string sort gets wrong), whatever the JSON's layout;
# never an rc, a leading zero, a pre-release, a draft or a release whose flags are missing.
d=$W/stable; make_release "$d" 26.3-0.1.1000000000; with_key "$d"
printf '[{"tag_name":"26.3-0.1.999999999","prerelease":false,"draft":false,"body":"a, \\"b\\": [c]"},' > "$d/page-1.json"
printf '{"tag_name":"nightly","prerelease":true,"draft":false}]\n' >> "$d/page-1.json"
cat > "$d/page-2.json" <<'PAGE'
[
  {
    "tag_name": "26.3-0.1.1000000000",
    "assets": [ { "name": "x", "draft": true } ],
    "draft": false,
    "prerelease": false
  },
  { "tag_name": "26.3-0.9.0", "draft": false, "prerelease": true },
  { "tag_name": "26.3-0.8.0", "draft": true, "prerelease": false },
  { "tag_name": "26.3-0.7.0", "prerelease": false },
  { "tag_name": "26.3-0.6.0", "draft": null, "prerelease": false },
  { "tag_name": "26.3-0.2.0-rc.1", "draft": false, "prerelease": false },
  { "tag_name": "26.3-01.0.0", "draft": false, "prerelease": false }
]
PAGE
printf '[]\n' > "$d/page-3.json"
run "$d" "$P" --version stable; verdict "--version stable: the newest published stable release, from a later page" 0 "Downloading Sunrise 26.3-0.1.1000000000 " "$d"

# 10b. A later page that fails, or that can't be read, stops the install rather than choosing from a short list.
d=$W/stable-short; cp -r "$W/stable" "$d"; rm -rf "$d/inst" "$d/bin" "$d/page-2.json"; with_key "$d"
run "$d" "$P" --version stable; verdict "--version stable: a failed page stops the install" 1 "could not list the releases on GitHub (page 2)" "$d"
[ ! -e "$d/inst/current" ] || { echo "FAIL installed from a short list"; fails=$((fails+1)); }
# A page that isn't a list of releases (cut short, `[null]`, `[,]`, a bare release object) stops it too, and an
# install already there stays on the release it had.
for bad in cut null comma object; do
  d=$W/stable-$bad; cp -r "$W/stable" "$d"; rm -rf "$d/inst" "$d/bin"; with_key "$d"
  case $bad in
    cut) head -c 60 "$W/stable/page-2.json" > "$d/page-2.json" ;;
    null) printf '[null]\n' > "$d/page-2.json" ;;
    comma) printf '[,]\n' > "$d/page-2.json" ;;
    object) printf '{"tag_name":"26.3-9.0.0","prerelease":false,"draft":false}\n' > "$d/page-2.json" ;;
  esac
  mkdir -p "$d/inst/releases/26.3-0.1.0"; ln -s "$d/inst/releases/26.3-0.1.0" "$d/inst/current"
  run "$d" "$P" --version stable; verdict "--version stable: a page that isn't a list ($bad) stops the install" 1 "could not read the releases GitHub listed (page 2)" "$d"
  [ "$(readlink "$d/inst/current")" = "$d/inst/releases/26.3-0.1.0" ] || { echo "FAIL the active install moved ($bad)"; fails=$((fails+1)); }
done
# The release sent for the chosen tag must be one release object, not a list holding it.
d=$W/stable-wrapped; cp -r "$W/stable" "$d"; rm -rf "$d/inst" "$d/bin"; with_key "$d"
{ printf '['; cat "$d/release.json"; printf ']\n'; } > "$d/tag-26.3-0.1.1000000000.json"
run "$d" "$P" --version stable; verdict "--version stable: a list sent as one release refused" 1 "could not read the release GitHub sent" "$d"

# 10c. The release served for the chosen tag names another one, or is no longer published: refused before any download.
d=$W/stable-swap; cp -r "$W/stable" "$d"; rm -rf "$d/inst" "$d/bin"; with_key "$d"
sed 's/26.3-0.1.1000000000/26.4-1.0.0/' "$d/release.json" > "$d/tag-26.3-0.1.1000000000.json"
run "$d" "$P" --version stable; verdict "--version stable: a release under another tag refused" 1 "names another tag" "$d"
d=$W/stable-draft; cp -r "$W/stable" "$d"; rm -rf "$d/inst" "$d/bin"; with_key "$d"
sed 's/"draft": false/"draft": null/' "$d/release.json" > "$d/tag-26.3-0.1.1000000000.json"
run "$d" "$P" --version stable; verdict "--version stable: a release whose draft flag isn't false refused" 1 "not published" "$d"

# 11. A Dev build is refused by name: servers don't install one.
d=$W/dev; make_release "$d" dev-2026-11-21-0c02067; with_key "$d"
run "$d" "$P" --version dev-2026-11-21-0c02067; verdict "a Dev build refused by name" 1 "is a Dev build" "$d"

echo "$fails case(s) failed"; [ "$fails" = 0 ]
