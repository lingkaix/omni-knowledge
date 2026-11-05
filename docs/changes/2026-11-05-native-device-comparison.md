# Native device identity comparison on Darwin

Hosted run 41994272561 stopped before macOS test inventory because `MetadataExt::dev()` is `u64`, while Darwin's native `dev_t` in the nofollow `statat` result is signed `i32`. Preserve the device/inode/current-regular-file comparison: convert Darwin's native device field using the same `as u64` representation as `MetadataExt`, and leave the Linux comparison unchanged. No pathname fallback, identity check removal or new authority is introduced.

This is a compilation correction, not evidence of macOS runtime behavior. Keep the original compile RED, the Linux native tracking `Recovery` failure and the two Git `Busy` failures separately; their owners are fixing those actual mechanisms. The fresh native run must execute the original controls, including real watch and child collection, before qualification.
