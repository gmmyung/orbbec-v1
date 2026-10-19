# Third-party materials

The unchanged `LICENSE.txt` is copied from the Orbbec release. Its SDK-specific
license continues to apply to the SDK binaries; this packaging does not grant
additional rights. The project owner chose to proceed with preparing redistribution.

## Source snapshots

- `libob_usb.so` reports libusb 1.0.26.11791 on the tested x86_64 host. The
  `sources/libusb-1.0.26.tar.bz2` upstream release includes build scripts and LGPL
  2.1 text. LGPL 2.1 applies to the upstream source.
- `liblive555.so` contains the version string 2022.04.26 in both SDK packages.
  The SDK notice instead lists 2019.08.13 under LGPL 2.1. The archived upstream
  2022.04.26 source uses LGPL 3.0 with GPL 3.0 terms incorporated. Both upstream
  license texts are included under `licenses/` without changing Orbbec's notice.
- Source origins and hashes are recorded in `provenance.json`. Extract the source
  archives to obtain all upstream source files, copyright notices, and build scripts.

These are upstream snapshots, not a claim that Orbbec's libraries were built
without patches. Exact vendor modifications, build configuration, and the license
basis for the live555 version discrepancy remain unverified. Before a public
binary release, establish corresponding source for the shipped LGPL components
and resolve the live555 notice discrepancy. A matching version string alone does
not establish corresponding source. Merely including these archives is not a
claim of complete LGPL compliance.

The SDK notice also lists MIT, BSD, Apache, zlib, and MPL dependencies. Preserve
that notice and honor the applicable source and attribution requirements when
redistributing. This package is prepared locally and has not been published.
