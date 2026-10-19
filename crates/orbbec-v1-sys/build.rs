use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=ORBBEC_V1_SDK_DIR");
    println!("cargo:rerun-if-env-changed=ORBBEC_V1_SYSROOT");
    println!("cargo:rerun-if-changed=wrapper.h");
    let target = env::var("TARGET").unwrap();
    let arch = match target.as_str() {
        "x86_64-unknown-linux-gnu" => "x64",
        "aarch64-unknown-linux-gnu" => "arm64",
        _ => panic!("Supported targets: x86_64-unknown-linux-gnu and aarch64-unknown-linux-gnu"),
    };
    let root = env::var_os("ORBBEC_V1_SDK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join(format!("../../.sdk/{arch}/OrbbecSDK_v1.10.37/SDK"))
        });
    let root = root.canonicalize().expect("SDK missing; run python3 scripts/fetch-sdk.py first (or set ORBBEC_V1_SDK_DIR to SDK/include's parent)");
    let include = root.join("include");
    let lib = root.join("lib");
    let binary = lib.join("libOrbbecSDK.so.1.10.37");
    let bytes = fs::read(&binary).expect("Expected pinned libOrbbecSDK.so.1.10.37");
    assert!(
        bytes.len() > 20 && &bytes[..4] == b"\x7fELF",
        "Invalid SDK ELF library"
    );
    assert_eq!(bytes[4], 2, "Expected ELF64 SDK");
    assert_eq!(bytes[5], 1, "Expected little-endian SDK");
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    assert_eq!(
        machine,
        if arch == "x64" { 62 } else { 183 },
        "SDK architecture differs from Cargo TARGET"
    );
    println!("cargo:rerun-if-changed={}", binary.display());
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=dylib=OrbbecSDK");
    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include.display()))
        .clang_arg(format!("--target={target}"))
        .allowlist_function("ob_.*")
        .allowlist_type("(ob_.*|OB.*)")
        .allowlist_var("OB_.*")
        .prepend_enum_name(false)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    if let Some(sysroot) = env::var_os("ORBBEC_V1_SYSROOT") {
        let sysroot = PathBuf::from(sysroot);
        builder = builder.clang_arg(format!("--sysroot={}", sysroot.display()));
        // Debian's cross libc package uses <sysroot>/include, rather than usr/include.
        if sysroot.join("include").is_dir() {
            builder = builder
                .clang_arg("-isystem")
                .clang_arg(sysroot.join("include").to_string_lossy());
        }
    }
    builder
        .generate()
        .expect("bindgen failed; install libclang and the target C headers")
        .write_to_file(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bindings.rs"))
        .unwrap();
}
