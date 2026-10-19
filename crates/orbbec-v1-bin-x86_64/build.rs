use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=sdk");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("sdk");
    println!("cargo:root={}", root.display());
}
