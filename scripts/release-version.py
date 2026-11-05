"""Validate immutable release metadata supplied as JSON on stdin; never edit files."""

import json
import sys
import tomllib


def release_version(files):
    package = tomllib.loads(files["Cargo.toml"])["package"]
    locked = [
        item
        for item in tomllib.loads(files["Cargo.lock"])["package"]
        if item["name"] == "jev-grep" and "source" not in item
    ]
    manifest = json.loads(files[".release-please-manifest.json"])
    if (
        package["name"] != "jev-grep"
        or len(locked) != 1
        or package["version"] != locked[0]["version"]
        or package["version"] != manifest["."]
    ):
        raise ValueError("Cargo and release manifest versions must agree")
    return package["version"]


if __name__ == "__main__":
    print(release_version(json.load(sys.stdin)))
