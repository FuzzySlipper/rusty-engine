#!/usr/bin/env bash
# Clean and incremental build times per candidate, each in a fresh target dir.
# Usage: measure-build.sh <scratch-dir>   (writes out/build-times.tsv)
set -euo pipefail
cd "$(dirname "$0")"
scratch=${1:?scratch dir}
out=out/build-times.tsv
printf 'package\tprofile\tclean_s\tincremental_s\tbinary_bytes\tstripped_bytes\n' > "$out"
for package in bootstrap-fixture bootstrap-raw bootstrap-kiss3d bootstrap-bevy; do
  case $package in
    bootstrap-fixture) source=fixture/src/main.rs; bin=bootstrap-baseline ;;
    *) source=${package#bootstrap-}/src/main.rs; bin=$package ;;
  esac
  for profile in dev release; do
    target="$scratch/$package-$profile"
    rm -rf "$target"
    dir=$([[ $profile == dev ]] && echo debug || echo release)
    start=$(date +%s.%N)
    cargo build -q --profile "$profile" -p "$package" --target-dir "$target"
    clean=$(echo "$(date +%s.%N) - $start" | bc)
    touch "$source"
    start=$(date +%s.%N)
    cargo build -q --profile "$profile" -p "$package" --target-dir "$target"
    incremental=$(echo "$(date +%s.%N) - $start" | bc)
    size=$(stat -c %s "$target/$dir/$bin")
    strip -o "$target/stripped" "$target/$dir/$bin"
    stripped=$(stat -c %s "$target/stripped")
    printf '%s\t%s\t%.1f\t%.1f\t%s\t%s\n' "$package" "$profile" "$clean" "$incremental" "$size" "$stripped" | tee -a "$out"
    rm -rf "$target"
  done
done
