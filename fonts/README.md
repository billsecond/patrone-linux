# PatroneOS's fonts

PatroneOS Sans, Mono, Document, Serif and Classic are drawn, built and
released in their own repository,
[billsecond/patroneos-fonts](https://github.com/billsecond/patroneos-fonts),
under the SIL Open Font License 1.1.

The `fonts-patroneos` package installs a release of them: the one
[`release.txt`](release.txt) names, which `packaging/build.sh` fetches and
checks against its SHA-256 before building the package. To move to a new
release, change its version, address and checksum there, and the package's
version (`packaging/fonts-patroneos/debian/changelog`).

`fonts-patrone-sans`, which installed Patrone Sans before them, is now a
transitional package that installs `fonts-patroneos`.
