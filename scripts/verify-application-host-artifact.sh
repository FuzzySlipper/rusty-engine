#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT="$REPO_ROOT/render/artifacts/application-host"
ARTIFACT_COPY="$(mktemp -d -t rusty-application-host-artifact.XXXXXX)"
trap 'rm -rf "$ARTIFACT_COPY"' EXIT

cp -a "$ARTIFACT/." "$ARTIFACT_COPY/"
pnpm --dir "$REPO_ROOT/render" run build:application-host-artifact
if ! diff -ru "$ARTIFACT_COPY" "$ARTIFACT"; then
  echo "checked application-host artifact does not match its reproducible build" >&2
  exit 1
fi

echo "application-host artifact freshness passed"
