"""Update only this binary's manifest and lockfile version, preserving dependencies."""
import re
import sys
import tomllib
from pathlib import Path


def set_version(version, root=Path('.')):
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('Expected a stable semantic version')
    manifest_path, lock_path = root / 'Cargo.toml', root / 'Cargo.lock'
    manifest, lock = manifest_path.read_text(), lock_path.read_text()
    package = tomllib.loads(manifest)['package']
    name, old = package['name'], package['version']
    entries = [p for p in tomllib.loads(lock)['package']
               if p['name'] == name and 'source' not in p]
    if len(entries) != 1 or entries[0]['version'] != old:
        raise ValueError('Manifest and lockfile versions must agree')

    manifest, manifest_count = re.subn(
        r'(\[package\]\n(?:(?!\[)[^\n]*\n)*?version = ")[^"]+("\n)',
        lambda m: m[1] + version + m[2], manifest, count=1)
    lock, lock_count = re.subn(
        rf'(\[\[package\]\]\nname = "{re.escape(name)}"\nversion = "){re.escape(old)}("\n)',
        lambda m: m[1] + version + m[2], lock)
    if manifest_count != 1 or lock_count != 1:
        raise ValueError('Expected exactly one package version in each Cargo file')
    # Validate both results before writing either file.
    assert tomllib.loads(manifest)['package']['version'] == version
    assert any(p['name'] == name and p['version'] == version
               for p in tomllib.loads(lock)['package'])
    manifest_path.write_text(manifest)
    lock_path.write_text(lock)


if __name__ == '__main__':
    set_version(sys.argv[1])
