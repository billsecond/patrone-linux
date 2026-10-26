//! Compiles the icons and style sheet into a GResource bundle, and the
//! settings schema, so `cargo run` works without installing anything.

use std::fmt::Write as _;
use std::process::Command;

fn main() {
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    println!("cargo:rerun-if-changed=data");

    // Every icon in data/icons, and the style sheet.
    let mut icons: Vec<String> = std::fs::read_dir("data/icons")
        .expect("read data/icons")
        .map(|entry| {
            entry
                .expect("read data/icons")
                .file_name()
                .into_string()
                .expect("UTF-8 name")
        })
        .filter(|name| name.ends_with(".svg"))
        .collect();
    icons.sort();
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  \
         <gresource prefix=\"/org/patrone/FileManager\">\n    \
         <file>style.css</file>\n    \
         <file>over-theme.css</file>\n",
    );
    for icon in &icons {
        writeln!(xml, "    <file>icons/{icon}</file>").unwrap();
    }
    xml.push_str("  </gresource>\n</gresources>\n");
    let manifest = format!("{out}/resources.gresource.xml");
    std::fs::write(&manifest, xml).expect("write the resource list");

    let status = Command::new("glib-compile-resources")
        .arg("--sourcedir=data")
        .arg(format!("--target={out}/resources.gresource"))
        .arg(&manifest)
        .status()
        .expect("glib-compile-resources (libglib2.0-dev-bin) is needed to build");
    assert!(status.success(), "glib-compile-resources failed");

    // Only for running from the build tree: the package installs the schema.
    let schemas = format!("{out}/schemas");
    std::fs::create_dir_all(&schemas).expect("create the schema folder");
    std::fs::copy(
        "data/org.patrone.FileManager.gschema.xml",
        format!("{schemas}/org.patrone.FileManager.gschema.xml"),
    )
    .expect("copy the schema");
    let status = Command::new("glib-compile-schemas")
        .arg("--strict")
        .arg(&schemas)
        .status()
        .expect("glib-compile-schemas (libglib2.0-bin) is needed to build");
    assert!(status.success(), "glib-compile-schemas failed");
}
