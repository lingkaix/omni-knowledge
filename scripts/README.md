# Local Build And Runtime Scripts

Read [the root guide](../README.md) and [toolchain](../docs/toolchain.md) first.
`scripts/bundle-macos.sh` builds the Rust release CLI and GPUI executable into a locally ad-hoc-signed app bundle under `dist/`.
It assembles a fresh bundle, signs the nested CLI before the outer app, and verifies the complete signature before replacement.
It preserves each previous bundle at the printed `dist/Omni Knowledge.previous-*` path, including any added files.
Ad-hoc signing needs no developer account and does not provide Developer ID or notarization for distribution.
The script does not install or publish the app.
`scripts/collect-licenses.py` uses development-time Python 3.9 or newer to collect locked Darwin dependency notices and exact MPL source archives.
It writes only a fresh destination and records upstream notice gaps; Python is not required by the app.
`scripts/setup-asr.sh` prepares a pinned optional whisper.cpp and multilingual model under ignored `temp/asr-runtime/`.
Read [the multimodal guide](../docs/multimodal.md) before setting the executable and model paths in a library configuration.
