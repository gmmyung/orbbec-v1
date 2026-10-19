use std::{env, path::PathBuf};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    assert_eq!(
        args.len(),
        3,
        "Usage: generate-orbbec-bindings TARGET SDK_DIR OUTPUT.rs"
    );
    let target = args[0].to_str().unwrap();
    assert!(matches!(
        target,
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu"
    ));
    let sdk = PathBuf::from(&args[1]).canonicalize().unwrap();
    let wrapper =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/orbbec-v1-sys/wrapper.h");
    let mut builder = bindgen::Builder::default()
        .header(wrapper.to_str().unwrap())
        .clang_arg(format!("-I{}", sdk.join("include").display()))
        .clang_arg(format!("--target={target}"))
        .allowlist_function("ob_.*")
        .allowlist_type("(ob_.*|OB.*)")
        .allowlist_var("OB_.*")
        .prepend_enum_name(false)
        .generate_comments(false);
    if let Some(sysroot) = env::var_os("ORBBEC_V1_SYSROOT") {
        let sysroot = PathBuf::from(sysroot);
        builder = builder.clang_arg(format!("--sysroot={}", sysroot.display()));
        if sysroot.join("include").is_dir() {
            builder = builder
                .clang_arg("-isystem")
                .clang_arg(sysroot.join("include").to_string_lossy());
        }
    }
    builder
        .generate()
        .expect("bindgen failed; install libclang and target C headers")
        .write_to_file(&args[2])
        .unwrap();
    assert!(
        std::process::Command::new("rustfmt")
            .args(["--edition", "2024"])
            .arg(&args[2])
            .status()
            .expect("Install the rustfmt component")
            .success(),
        "Formatting generated bindings failed"
    );
}
