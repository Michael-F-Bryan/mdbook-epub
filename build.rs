//! A small build script for finding the version of `mdbook` this crate is
//! compiled against and injecting it into the compiled crate for use with the
//! `env!()` macro.

extern crate cargo;

use std::env;
use std::path::Path;
use std::error::Error;

use cargo::core::Workspace;
use cargo::util::Config;
use cargo::ops;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let version = find_dependency_version(&manifest_dir, "mdbook").unwrap();

    println!("cargo:rustc-env=MDBOOK_VERSION={}", version);
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
}

/// Find the version of the provided dependency as reported by `cargo`.
fn find_dependency_version<P: AsRef<Path>>(
    manifest_dir: P,
    dep: &str,
) -> Result<String, Box<dyn Error>> {
    let config = Config::default()?;

    let manifest = manifest_dir.as_ref().join("Cargo.toml");
    let ws = Workspace::new(&manifest, &config)?;

    let (_, resolve) = ops::resolve_ws(&ws)?;

    let mdbook = resolve.query(dep)?;

    Ok(mdbook.version().to_string())
}
