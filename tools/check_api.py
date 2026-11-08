#!/usr/bin/env python3
"""Type-check the trait mod against a Horizon source tree, as the launcher stages it.

    git -C ../horizon archive <rev> | tar -x -C target/horizon-src
    python3 tools/check_api.py target/horizon-src [--features player-break] [--clippy]

The api package is copied into the tree's workspace as `native-api/ultimine`
(the launcher's layout) with its horizon-client dependency pointed at the
tree, then `cargo check` (or clippy) runs on it alone, offline, with its own
target directory under target/. Only the given tree is changed.
"""
import argparse
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
API = ROOT / 'mixins/ultimine/api'


def stage(tree: Path) -> None:
    dest = tree / 'native-api/ultimine'
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(API, dest, ignore=shutil.ignore_patterns('target'))
    manifest = dest / 'Cargo.toml'
    text = manifest.read_text()
    text, n = re.subn(r'^horizon-client = \{[^\n]*\}$',
                      'horizon-client = { path = "../../crates/horizon-client", default-features = false }',
                      text, flags=re.M)
    assert n == 1, 'api Cargo.toml: horizon-client dependency not found'
    manifest.write_text(text)
    workspace = tree / 'Cargo.toml'
    root = workspace.read_text()
    if '"native-api/ultimine"' not in root:
        root, n = re.subn(r'^members = \[', 'members = [\n    "native-api/ultimine",', root, count=1, flags=re.M)
        assert n == 1, 'workspace members not found'
        workspace.write_text(root)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument('tree', type=Path)
    parser.add_argument('--features', default='')
    parser.add_argument('--clippy', action='store_true')
    args = parser.parse_args()
    stage(args.tree)
    command = ['cargo', 'clippy' if args.clippy else 'check', '--offline', '-p', 'mod-ultimine', '--all-targets']
    if args.clippy:
        command.append('--no-deps')
    if args.features:
        command += ['--features', args.features]
    if args.clippy:
        command += ['--', '-D', 'warnings']
    env_target = ROOT / 'target/api-check'
    subprocess.run(['nice', '-n', '19', 'ionice', '-c3', *command], cwd=args.tree, check=True,
                   env={**__import__('os').environ, 'CARGO_TARGET_DIR': str(env_target)})


if __name__ == '__main__':
    main()
