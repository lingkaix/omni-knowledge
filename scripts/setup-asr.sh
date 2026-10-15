#!/bin/bash
# Build a pinned local Metal whisper.cpp CLI and verify its multilingual base model.
# Requires macOS arm64, CMake, Apple command-line tools, curl, and Python 3.
# All downloaded and generated files stay in ignored temp/asr-runtime; no system install occurs.
set -euo pipefail

OMNI_ASR_PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
OMNI_ASR_ROOT="$OMNI_ASR_PROJECT_ROOT/temp/asr-runtime"
OMNI_ASR_COMMIT="d1be6fde11ac6e0407606b4e42fe72d34add8037"
OMNI_ASR_ARCHIVE_SHA256="645445da884df6dbd52b30f82d7be7c01d1767c938714e477e786b88d529e9f4"
OMNI_ASR_MODEL_REVISION="5359861c739e955e79d9a303bcbc70fb988958b1"
OMNI_ASR_MODEL_SHA256="60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe"
OMNI_ASR_SOURCE="$OMNI_ASR_ROOT/whisper.cpp-$OMNI_ASR_COMMIT"
OMNI_ASR_ARCHIVE="$OMNI_ASR_ROOT/whisper-source.tar.gz"
OMNI_ASR_MODEL="$OMNI_ASR_ROOT/models/ggml-base.bin"

if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
    printf '%s\n' 'This ASR setup supports Apple Silicon macOS only.' >&2
    exit 1
fi
for OMNI_ASR_TOOL in cmake curl python3 cc; do
    command -v "$OMNI_ASR_TOOL" >/dev/null || { printf 'Missing prerequisite: %s\n' "$OMNI_ASR_TOOL" >&2; exit 1; }
done
python3 - "$OMNI_ASR_PROJECT_ROOT" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
for relative in ('temp', 'temp/asr-runtime', 'temp/asr-runtime/models', 'temp/asr-runtime/build'):
    path = root / relative
    if path.is_symlink():
        raise SystemExit(f'Refusing a symlinked runtime path: {path}')
    if path.exists() and not path.is_dir():
        raise SystemExit(f'Runtime directory collides with a file: {path}')
runtime = root / 'temp/asr-runtime'
if runtime.exists():
    for path in runtime.rglob('*'):
        if path.is_symlink():
            raise SystemExit(f'Refusing a symlinked runtime artifact: {path}')
PY
mkdir -p "$OMNI_ASR_ROOT/models"
mkdir "$OMNI_ASR_ROOT/.setup-lock" || { printf '%s\n' 'Another ASR setup owns .setup-lock; wait for it to finish.' >&2; exit 1; }
trap 'rmdir "$OMNI_ASR_ROOT/.setup-lock"' EXIT

if [[ ! -f "$OMNI_ASR_ARCHIVE" ]]; then
    curl --disable --fail --location --silent --show-error --proto '=https' --proto-redir '=https' \
        --retry 2 --connect-timeout 15 --max-time 120 \
        --output "$OMNI_ASR_ARCHIVE.part" \
        "https://codeload.github.com/ggml-org/whisper.cpp/tar.gz/$OMNI_ASR_COMMIT"
    mv "$OMNI_ASR_ARCHIVE.part" "$OMNI_ASR_ARCHIVE"
fi
python3 - "$OMNI_ASR_ROOT" "$OMNI_ASR_ARCHIVE_SHA256" "$OMNI_ASR_COMMIT" <<'PY'
from pathlib import Path
import hashlib, sys, tarfile
root = Path(sys.argv[1])
archive_path = root / 'whisper-source.tar.gz'
if archive_path.is_symlink():
    raise SystemExit('Source archive must not be a symlink')
if hashlib.sha256(archive_path.read_bytes()).hexdigest() != sys.argv[2]:
    raise SystemExit('Pinned source archive SHA-256 mismatch; existing files are preserved')
prefix = f'whisper.cpp-{sys.argv[3]}'
source = root / prefix
with tarfile.open(archive_path, 'r:gz') as archive:
    members = archive.getmembers()
    for member in members:
        relative = Path(member.name)
        if relative.is_absolute() or '..' in relative.parts or relative.parts[0] != prefix:
            raise SystemExit('Source archive contains an unsafe path')
        if not (member.isfile() or member.isdir()):
            raise SystemExit('Source archive contains a link or special file')
    if source.exists():
        for member in members:
            path = root / member.name
            if path.is_symlink():
                raise SystemExit(f'Pinned source path became a symlink: {path}')
            if member.isfile():
                if not path.is_file() or path.read_bytes() != archive.extractfile(member).read():
                    raise SystemExit(f'Pinned source changed; preserving it and stopping: {path}')
            elif not path.is_dir():
                raise SystemExit(f'Pinned source directory is missing: {path}')
    else:
        archive.extractall(root)
PY

if [[ ! -f "$OMNI_ASR_MODEL" ]]; then
    curl --disable --fail --location --silent --show-error --proto '=https' --proto-redir '=https' \
        --retry 2 --connect-timeout 15 --max-time 600 \
        --output "$OMNI_ASR_MODEL.part" \
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/$OMNI_ASR_MODEL_REVISION/ggml-base.bin"
    mv "$OMNI_ASR_MODEL.part" "$OMNI_ASR_MODEL"
fi
python3 - "$OMNI_ASR_MODEL" "$OMNI_ASR_MODEL_SHA256" <<'PY'
from pathlib import Path
import hashlib, sys
path = Path(sys.argv[1])
if path.is_symlink() or not path.is_file() or path.stat().st_size != 147951465:
    raise SystemExit('Pinned base model file or size is invalid')
checksum = hashlib.sha256()
with path.open('rb') as model:
    for chunk in iter(lambda: model.read(1024 * 1024), b''):
        checksum.update(chunk)
if checksum.hexdigest() != sys.argv[2]:
    raise SystemExit('Pinned base model SHA-256 mismatch; existing files are preserved')
PY

OMNI_ASR_BUILD_STARTED=$SECONDS
cmake -S "$OMNI_ASR_SOURCE" -B "$OMNI_ASR_ROOT/build" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES=arm64 -DBUILD_SHARED_LIBS=OFF \
    -DWHISPER_BUILD_IS_DEV=OFF -DWHISPER_BUILD_COMMIT=d1be6fde \
    -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_EXAMPLES=ON -DWHISPER_BUILD_SERVER=OFF \
    -DWHISPER_CURL=OFF -DWHISPER_COREML=OFF \
    -DGGML_METAL=ON -DGGML_METAL_EMBED_LIBRARY=ON -DGGML_NATIVE=OFF \
    -DGGML_OPENMP=OFF -DGGML_CPU_KLEIDIAI=OFF -DGGML_CCACHE=OFF
cmake --build "$OMNI_ASR_ROOT/build" --target whisper-cli --config Release --parallel 8
OMNI_ASR_BUILD_SECONDS=$((SECONDS - OMNI_ASR_BUILD_STARTED))
python3 - "$OMNI_ASR_ROOT" "$OMNI_ASR_COMMIT" "$OMNI_ASR_ARCHIVE_SHA256" \
    "$OMNI_ASR_MODEL_REVISION" "$OMNI_ASR_MODEL_SHA256" "$OMNI_ASR_BUILD_SECONDS" <<'PY'
from pathlib import Path
import json, sys
root = Path(sys.argv[1])
binary = root / 'build/bin/whisper-cli'
model = root / 'models/ggml-base.bin'
record = {
    'whisper_version': '1.9.5', 'source_commit': sys.argv[2],
    'source_archive_sha256': sys.argv[3], 'model_revision': sys.argv[4],
    'model_sha256': sys.argv[5], 'last_build_seconds': int(sys.argv[6]),
    'metal': True, 'coreml': False, 'binary_bytes': binary.stat().st_size,
    'model_bytes': model.stat().st_size,
    'whisper_executable': str(binary), 'whisper_model': str(model),
}
(root / 'setup-manifest.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'whisper_executable': str(binary), 'whisper_model': str(model)}, indent=2))
PY
