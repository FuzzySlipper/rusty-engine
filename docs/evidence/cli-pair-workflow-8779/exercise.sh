#!/usr/bin/env bash
# #8779 evidence: a fresh HOME with only .NET, curl and tar, the published
# bootstrap, and the template migrated to the CLI workflow.
set -uo pipefail
S=${SCRATCH:?set SCRATCH to an empty scratch directory}
F=$S/fresh
rm -rf "$F"; mkdir -p "$F/home"
BASEPATH="$F/home/.local/bin:/home/agent/.dotnet:/usr/bin:/bin"
run() { echo; echo "\$ $*"; env -i HOME="$F/home" PATH="$BASEPATH" TMPDIR="$F/tmp" "$@"; echo "[exit $?]"; }
mkdir -p "$F/tmp"
probe() { # port
  for _ in $(seq 1 120); do curl -sf -o /dev/null "http://127.0.0.1:$1/product-bootstrap.json" && break; sleep 0.5; done
  echo "probe: $(curl -s "http://127.0.0.1:$1/product-bootstrap.json" | head -c 110)"
}
stop_dev() { # pattern for the pair CLI process
  local pid; pid=$(pgrep -f "$1" | head -n 1)
  [[ -n "$pid" ]] && kill -TERM "$pid"; sleep 4
}

echo "## 1. Bootstrap from the published route"
echo "\$ curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash"
curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh \
  | env -i HOME="$F/home" PATH="$BASEPATH" TMPDIR="$F/tmp" bash; echo "[exit $?]"
run rusty --help

echo; echo "## 2. A product from the migrated template, at its existing pin"
git clone -q -b cli-workflow-8779 /home/agent/dev/rusty-template "$F/product"
cd "$F/product"
P=src/RustyTemplate.Game/RustyTemplate.Game.csproj
run rusty status
run rusty install
run rusty build --project $P
( env -i HOME="$F/home" PATH="$BASEPATH" TMPDIR="$F/tmp" setsid rusty dev --project $P --port 8951 > "$F/dev-a.log" 2>&1 & )
probe 8951; grep -o '"event":"[a-z-]*"' "$F/dev-a.log" | head -3
stop_dev "[f]resh/home/.cache/rusty-engine/pairs/.*/bin/rusty dev"

echo; echo "## 3. Explicit update"
run rusty update --check
run rusty update
echo "\$ git diff"; git diff
run rusty build --project $P
( env -i HOME="$F/home" PATH="$BASEPATH" TMPDIR="$F/tmp" setsid rusty dev --project $P --port 8952 > "$F/dev-b.log" 2>&1 & )
probe 8952; grep -o '"event":"[a-z-]*"' "$F/dev-b.log" | head -3
stop_dev "[f]resh/home/.cache/rusty-engine/pairs/.*/bin/rusty dev"

echo; echo "## 4. Offline (release server and every proxy unreachable)"
OFF=(http_proxy=http://127.0.0.1:9 https_proxy=http://127.0.0.1:9 HTTP_PROXY=http://127.0.0.1:9 HTTPS_PROXY=http://127.0.0.1:9 ALL_PROXY=http://127.0.0.1:9 RUSTY_ENGINE_RELEASES=http://127.0.0.1:9/releases)
rm -rf src/*/obj src/*/bin
run env "${OFF[@]}" rusty status
run env "${OFF[@]}" rusty install
run env "${OFF[@]}" rusty build --project $P
( env -i HOME="$F/home" PATH="$BASEPATH" TMPDIR="$F/tmp" "${OFF[@]}" setsid rusty dev --project $P --port 8953 > "$F/dev-c.log" 2>&1 & )
probe 8953; grep -o '"event":"[a-z-]*"' "$F/dev-c.log" | head -3
stop_dev "[f]resh/home/.cache/rusty-engine/pairs/.*/bin/rusty dev"
run env "${OFF[@]}" rusty update --check

echo; echo "## 5. Missing and mismatched prerequisites"
mkdir -p "$F/shim"; printf '#!/bin/sh\necho 8.0.404\n' > "$F/shim/dotnet"; chmod +x "$F/shim/dotnet"
echo; echo "\$ rusty status   # an old dotnet first on PATH"
env -i HOME="$F/home" PATH="$F/shim:$BASEPATH" rusty status; echo "[exit $?]"
echo; echo "\$ rusty status   # no dotnet on PATH"
env -i HOME="$F/home" PATH="$F/home/.local/bin:/usr/bin:/bin" rusty status; echo "[exit $?]"
echo; echo "\$ rusty build --project $P   # no dotnet on PATH"
env -i HOME="$F/home" PATH="$F/home/.local/bin:/usr/bin:/bin" rusty build --project $P; echo "[exit $?]"
