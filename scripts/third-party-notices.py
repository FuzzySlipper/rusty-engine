#!/usr/bin/env python3
"""Print the licence and notice files of the third-party crates
rusty-product-host links with the given features, and of the native
libraries those crates build in. Identical texts are printed once, with the
crates they apply to; a crate that ships no such file is listed with its
licence expression and repository.

usage: third-party-notices.py [--features <a,b>]
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

PACKAGE = "csharp-product-runtime"
NOTICE = re.compile(r"^(licen[cs]e|copying|notice|copyright|unlicense|credits)", re.I)
# Directories in a crate's source that hold no built code.
SKIPPED = {".git", "test", "tests", "testing", "examples", "docs", "doc", "benches", "python"}


def cargo(*args):
    return subprocess.run(["cargo", *args], check=True, capture_output=True, encoding="utf-8").stdout


def notice_files(directory, nested):
    """The crate's notice files; with `nested`, those of its bundled sources too."""
    for root, dirs, files in os.walk(directory):
        dirs[:] = sorted(d for d in dirs if d.lower() not in SKIPPED)
        for name in sorted(files):
            if NOTICE.match(name):
                yield os.path.join(root, name)
        if not nested:
            return


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--features", default="")
    features = parser.parse_args().features
    feature_args = ["--features", features] if features else []
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    vendor = os.path.join(root, "rust", "vendor")

    tree = cargo("tree", "--locked", "-p", PACKAGE, *feature_args, "-e", "normal",
                 "--prefix", "none", "--format", "{p}")
    linked = set()
    for line in tree.splitlines():
        name, version = line.split()[:2]
        linked.add((name, version.lstrip("v")))
    metadata = json.loads(cargo("metadata", "--locked", "--format-version", "1", "--all-features"))

    texts = {}  # digest -> (text, [who])
    bare = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if (package["name"], package["version"]) not in linked:
            continue
        directory = os.path.dirname(package["manifest_path"])
        # The Engine's own crates are not third-party; its vendored ones are.
        if package["source"] is None and not directory.startswith(vendor):
            continue
        label = f"{package['name']} {package['version']}"
        found = False
        for path in notice_files(directory, nested=bool(package.get("links"))):
            with open(path, "rb") as file:
                data = file.read()
            text = data.decode("utf-8", errors="replace").replace("\r\n", "\n").strip()
            if not text:
                continue
            found = True
            relative = os.path.relpath(path, directory).replace(os.sep, "/")
            digest = hashlib.sha256(text.encode()).hexdigest()
            texts.setdefault(digest, (text, []))[1].append(f"{label} ({relative})")
        if not found:
            bare.append(f"{label}: {package.get('license') or 'no licence expression'}"
                        f"{', ' + package['repository'] if package.get('repository') else ''}")

    out = sys.stdout
    # Windows writes the console code page and CRLF otherwise.
    out.reconfigure(encoding="utf-8", newline="\n")
    out.write("Third-party notices for rusty-product-host"
              f"{' (features ' + features + ')' if features else ''}.\n"
              "Each text below applies to the crates, and the sources they build in, "
              "listed above it.\n")
    for text, who in sorted(texts.values(), key=lambda entry: entry[1][0]):
        out.write("\n" + "=" * 78 + "\n")
        out.write("".join(f"{line}\n" for line in who))
        out.write("-" * 78 + "\n" + text + "\n")
    if bare:
        out.write("\n" + "=" * 78 + "\n")
        out.write("Crates that ship no licence file, under the licence each one names:\n")
        out.write("-" * 78 + "\n")
        out.write("".join(f"{line}\n" for line in bare))


if __name__ == "__main__":
    main()
