#!/usr/bin/env python3
"""Produce a deterministic, source-only `.hnmod` of the Ultimine trait mod."""
import hashlib
import json
from pathlib import Path
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / 'mixins/ultimine'
# horizon-mixins revision recorded in the manifest (the launcher's own pin at the
# Horizon revision below; a trait mod with no injections never runs it).
MIXINS = '5a64af938ec86b00d53d16a408aabc0038fb3e57'


def manifest():
    mixin = tomllib.loads((BUNDLE / 'horizon-mixin.toml').read_text())
    api = tomllib.loads((BUNDLE / 'api/Cargo.toml').read_text())['package']
    engine = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
    version = mixin['mod']['version']
    assert version == api['version'] == engine['version'], 'versions disagree'
    rev = mixin['compatibility']['horizon_revision']
    dep = tomllib.loads((BUNDLE / 'api/Cargo.toml').read_text())['dependencies']['horizon-client']
    assert dep.get('rev') == rev, 'api crate and bundle pin different Horizon revisions'
    return dict(schema=1, id=mixin['mod']['id'], version=version, game='26.3', horizon=rev, mixins=MIXINS)


def files():
    out = {}
    for path in sorted(BUNDLE.rglob('*')):
        if path.is_file() and 'target' not in path.relative_to(BUNDLE).parts:
            assert path.suffix in ('.rs', '.toml'), f'unexpected bundle file {path}'
            out[str(path.relative_to(ROOT))] = path.read_bytes()
    return out


def package():
    meta = manifest()
    content = {'horizon-native.json': (json.dumps(meta, indent=2, sort_keys=True) + '\n').encode()}
    content.update(files())
    out = ROOT / 'target' / f"{meta['id']}-{meta['version']}.hnmod"
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(content.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
    print(json.dumps(dict(file=str(out), size=out.stat().st_size,
                          sha256=hashlib.sha256(out.read_bytes()).hexdigest()), indent=2))


if __name__ == '__main__':
    package()
