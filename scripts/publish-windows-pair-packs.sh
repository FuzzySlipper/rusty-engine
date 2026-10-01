#!/usr/bin/env bash
set -euo pipefail

# Build and publish a published pair's win-x64 runtime pair and desktop pack
# on a Windows build machine reached over SSH, then add them to the pair's
# release and pair-release.json (`targets."win-x64"`).
#
# The Linux pair is published by CI first; this adds Windows beside it. The
# Windows pair reuses the Linux pair's SDK package (it is the same on every
# target) and builds its runtime packs at the pair's exact source revision.
# The machine needs the MSVC build tools, Git Bash, Rust, Node/pnpm, cmake and
# ninja, and a clone of this repository (see crew-services
# docs/playtest-windows.md). Run from a machine with `gh` access to the repo.
#
# usage: scripts/publish-windows-pair-packs.sh --host <ssh-host> --checkout <C:/path> [--version <pair>]

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=FuzzySlipper/rusty-engine
host=""
checkout=""
version=""
usage() { echo "usage: scripts/publish-windows-pair-packs.sh --host <ssh-host> --checkout <C:/path> [--version <pair>]" >&2; }
while (($#)); do
    case "$1" in
        --host) host=${2:?}; shift 2 ;;
        --checkout) checkout=${2:?}; shift 2 ;;
        --version) version=${2:?}; shift 2 ;;
        *) usage; exit 2 ;;
    esac
done
[[ -n "$host" && -n "$checkout" ]] || { usage; exit 2; }
if [[ -z "$version" ]]; then
    version=$(gh api "repos/$repo/releases/latest" --jq .tag_name)
    version=${version#csharp-sdk-v}
fi
tag="csharp-sdk-v$version"
pair="rusty-engine-csharp-pair-$version"
desktop="rusty-engine-desktop-pack-$version"

if gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name' | grep -qx "$pair-win-x64.tar.gz"; then
    echo "$tag already has $pair-win-x64.tar.gz" >&2
    exit 1
fi

work=$(mktemp -d -t rusty-windows-pair.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
gh release download "$tag" --repo "$repo" --dir "$work" --pattern "$pair-linux-x64.tar.gz*" --pattern pair-release.json
(cd "$work" && sha256sum --check --status "$pair-linux-x64.tar.gz.sha256")
revision=$(jq -r .sourceRevision "$work/pair-release.json")
tar -xzf "$work/$pair-linux-x64.tar.gz" -C "$work" "$pair-linux-x64/sdk-feed"

# The build runs in Git Bash inside the MSVC developer environment, with
# MSVC's link.exe ahead of Git's own `link`.
remote="C:/Users/$(ssh "$host" '$env:USERNAME' | tr -d '\r')/rusty-windows-pair/$version"
# Git Bash paths: the pair scripts take a path not starting with / as relative.
drive=${remote%%:*}
posix="/${drive,,}${remote#*:}"
cat > "$work/build.sh" <<EOF
set -euo pipefail
export PATH="\$(dirname "\$(command -v cl)"):\$HOME/AppData/Local/Microsoft/WinGet/Links:/c/Program Files/nodejs:\$HOME/AppData/Local/pnpm:\$PATH"
cd "$checkout"
git fetch -q origin
git checkout -q --force "$revision"
git clean -q -fd
pnpm install --frozen-lockfile --reporter=silent
rm -rf "$posix/out"
scripts/build-csharp-release-pair.sh --sdk-feed "$posix/sdk-feed" --output "$posix/out/pair"
scripts/build-desktop-runtime-pack-archive.sh --output "$posix/out/desktop"
EOF
cat > "$work/build.ps1" <<'EOF'
# A .cmd file keeps cmd's quote stripping away from the paths with spaces.
$vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath
Set-Content -Encoding ascii "$PSScriptRoot\build.cmd" ("@call `"$vs\VC\Auxiliary\Build\vcvars64.bat`" >nul`r`n" +
    "@`"$env:ProgramFiles\Git\bin\bash.exe`" `"$PSScriptRoot/build.sh`"")
& cmd /c "$PSScriptRoot\build.cmd"
exit $LASTEXITCODE
EOF
ssh "$host" "Remove-Item -Recurse -Force '$remote' -ErrorAction SilentlyContinue; New-Item -Force -ItemType Directory '$remote' | Out-Null"
scp -q -r "$work/$pair-linux-x64/sdk-feed" "$work/build.sh" "$work/build.ps1" "$host:$remote/"
ssh "$host" "powershell -NoProfile -ExecutionPolicy Bypass -File '$remote/build.ps1'"
mkdir "$work/out"
scp -q "$host:$remote/out/pair/$pair-win-x64.tar.gz" "$host:$remote/out/pair/$pair-win-x64.tar.gz.sha256" \
    "$host:$remote/out/desktop/$desktop-win-x64.tar.xz" "$host:$remote/out/desktop/$desktop-win-x64.tar.xz.sha256" "$work/out/"
(cd "$work/out" && sha256sum --check --status "$pair-win-x64.tar.gz.sha256" "$desktop-win-x64.tar.xz.sha256")

asset() {
    jq -n --arg name "$1" --arg sha256 "$(sha256sum "$work/out/$1" | awk '{print $1}')" \
        --argjson bytes "$(wc -c < "$work/out/$1" | tr -d '[:space:]')" \
        --arg url "https://github.com/$repo/releases/download/$tag/$1" '{name: $name, sha256: $sha256, bytes: $bytes, url: $url}'
}
jq --argjson archive "$(asset "$pair-win-x64.tar.gz")" --argjson desktop "$(asset "$desktop-win-x64.tar.xz")" \
    '.targets["win-x64"] = {archive: $archive, desktopPack: $desktop}' "$work/pair-release.json" > "$work/out/pair-release.json"
gh release upload "$tag" --repo "$repo" "$work/out/$pair-win-x64.tar.gz" "$work/out/$pair-win-x64.tar.gz.sha256" \
    "$work/out/$desktop-win-x64.tar.xz" "$work/out/$desktop-win-x64.tar.xz.sha256"
gh release upload "$tag" --repo "$repo" --clobber "$work/out/pair-release.json"
printf 'published win-x64 pair and desktop pack for %s\n' "$tag"
