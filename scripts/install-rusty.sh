#!/usr/bin/env bash
# Install or refresh the `rusty` command, the Rusty Engine product workflow.
#
#   curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash
#   curl -fsSL .../install-rusty.sh | bash -s -- --version 0.1.0-dev.abc123def456
#
# Downloads the newest published SDK/runtime pair (or --version), checks its
# SHA-256, puts that pair's `rusty` in ~/.local/bin (or RUSTY_BIN_DIR), and
# hands the same archive to `rusty install --archive`, so the pair also lands
# in the shared cache. It never changes a product's pin. Everything after this
# is `rusty --help`.
set -euo pipefail

releases=${RUSTY_ENGINE_RELEASES:-https://github.com/FuzzySlipper/rusty-engine/releases}
releases=${releases%/}
bin_dir=${RUSTY_BIN_DIR:-$HOME/.local/bin}
version=""

usage() {
    echo "usage: install-rusty.sh [--version <pair-version>]" >&2
}

while (($#)); do
    case "$1" in
        --version)
            (($# >= 2)) || { usage; exit 2; }
            version=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
done

[[ "$(uname -sm)" == "Linux x86_64" ]] || {
    echo "install-rusty: published pairs target Linux x86_64; this machine is $(uname -sm)." >&2
    exit 1
}
for tool in curl tar sha256sum; do
    command -v "$tool" >/dev/null || {
        echo "install-rusty: $tool is required." >&2
        exit 1
    }
done

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

if [[ -z "$version" ]]; then
    curl -fsSL --retry 3 -o "$work/pair-release.json" "$releases/latest/download/pair-release.json"
    version=$(grep -o '"version": *"[^"]*"' "$work/pair-release.json" | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/')
    [[ -n "$version" ]] || {
        echo "install-rusty: the latest pair-release.json names no version." >&2
        exit 1
    }
fi

name="rusty-engine-csharp-pair-$version-linux-x64"
archive="$name.tar.gz"
url="$releases/download/csharp-sdk-v$version/$archive"
echo "install-rusty: downloading Engine pair $version"
curl -fsSL --retry 3 -o "$work/$archive.sha256" "$url.sha256"
curl -fsSL --retry 3 -o "$work/$archive" "$url"
(cd "$work" && sha256sum --check --quiet "$archive.sha256")

tar -xzf "$work/$archive" -C "$work" "$name/runtime-pack/bin/rusty"
mkdir -p "$bin_dir"
install -m 755 "$work/$name/runtime-pack/bin/rusty" "$bin_dir/.rusty.incoming.$$"
mv -f "$bin_dir/.rusty.incoming.$$" "$bin_dir/rusty"
"$bin_dir/rusty" install --archive "$work/$archive"

echo "install-rusty: installed $bin_dir/rusty from pair $version."
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "install-rusty: add $bin_dir to PATH to run \`rusty\` by name." ;;
esac
echo "Next, in a product repository: rusty status"
