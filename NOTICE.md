# Third party SDK

The Rust wrapper and project scripts are MIT licensed. Orbbec SDK binaries are
downloaded separately into the ignored `.sdk/` directory and keep their own
license. This project's MIT license does not replace the SDK license.

Read the `LICENSE.txt` inside the downloaded package before redistributing SDK
files. Orbbec's library terms restrict use to Orbbec products and prohibit
modifying, decompiling, or reverse engineering the library. The package also
contains notices for third party dependencies. Preserve applicable notices when
distributing an application with these libraries.

Official source: https://github.com/orbbec/OrbbecSDK/blob/main/LICENSE.txt

Release packages and SHA256 values are pinned in `scripts/sdk-lock.json`.
Generated bindings are produced locally from those packages' public C headers.
