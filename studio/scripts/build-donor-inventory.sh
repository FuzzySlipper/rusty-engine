#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <asha-studio-checkout> <commit>" >&2
  exit 2
fi

DONOR_ROOT="$1"
COMMIT="$2"
GIT_DIR="$DONOR_ROOT/.git"

if [[ ! -d "$GIT_DIR" ]]; then
  echo "donor git directory does not exist: $GIT_DIR" >&2
  exit 1
fi

git --git-dir="$GIT_DIR" cat-file -e "$COMMIT^{commit}"
printf 'mode\tblob\tpath\n'
git --git-dir="$GIT_DIR" ls-tree -r "$COMMIT" |
  while IFS=$'\t' read -r metadata path; do
    read -r mode object_type blob <<< "$metadata"
    if [[ "$object_type" != "blob" ]]; then
      echo "unexpected donor object type for $path: $object_type" >&2
      exit 1
    fi
    printf '%s\t%s\t%s\n' "$mode" "$blob" "$path"
  done
