//! Links libarchive by its run-time name, libarchive.so.13, so the build
//! needs only the library Ubuntu's desktop already has (libarchive13t64),
//! not its development package. Its soname has been 13 since 2012.

fn main() {
    println!("cargo:rustc-link-lib=dylib:+verbatim=libarchive.so.13");
}
