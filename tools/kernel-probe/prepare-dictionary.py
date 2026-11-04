#!/usr/bin/env python3
"""Supply the pinned IPADIC archive before Cargo runs its build script.

Adapted from OpenKit scripts/retrieval-dictionary.py at
b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1; only the default path changes.
The Apache-2.0 license is retained in vendor/retrieval-kernel/LICENSE.
"""
import argparse
import hashlib
from pathlib import Path
import tarfile
import shutil
import os
import fcntl

URL = 'https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz'
SHA256 = 'a7ba9f645ffe7094e56ae1c4a81d100df8fbb1e28bbe1792622e9728e162db3d'
CACHE_VERSION = '5.3.0-fmt2'


def supply(cache: Path, supplied: Path, notice: Path | None) -> None:
    """Verify vendored supply and reuse the cached archive, then retain the exact license."""
    archive = cache / CACHE_VERSION / URL.rsplit('/', 1)[1]
    verify(supplied)
    archive.parent.mkdir(parents=True, exist_ok=True)
    with (cache / '.supply.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not archive.exists():
            temporary = archive.with_suffix(f'.{os.getpid()}.stage')
            shutil.copyfile(supplied, temporary)
            verify(temporary)
            temporary.replace(archive)
    verify(archive)
    if notice:
        with tarfile.open(archive, 'r:gz') as bundle:
            names = [item for item in bundle.getmembers()
                     if item.name.rsplit('/', 1)[-1] == 'COPYING' and item.isfile()]
            if len(names) != 1 or names[0].size > 64 * 1024:
                raise ValueError('Retrieval Kernel: pinned dictionary COPYING notice unavailable')
            source = bundle.extractfile(names[0])
            if source is None:
                raise ValueError('Retrieval Kernel: dictionary notice unreadable')
            notice.parent.mkdir(parents=True, exist_ok=True)
            notice.write_bytes(source.read())
    print(f'IPADIC {CACHE_VERSION}: verified {SHA256}')


def verify(archive: Path) -> None:
    """Reject changed supply before the Lindera build script can observe it."""
    with archive.open('rb') as source:
        actual = hashlib.file_digest(source, 'sha256').hexdigest()
    if actual != SHA256:
        raise ValueError('Retrieval Kernel: IPADIC archive SHA-256 mismatch')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--archive', type=Path, default=Path(__file__).resolve().parents[2] / 'vendor/retrieval-kernel/vendor/mecab-ipadic-2.7.0-20250920.tar.gz')
    parser.add_argument('--notice', type=Path)
    args = parser.parse_args()
    supply(args.cache, args.archive, args.notice)
