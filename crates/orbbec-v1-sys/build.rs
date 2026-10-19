use std::{env, fs, path::PathBuf};

const LIBRARIES: &[&str] = &[
    "libOrbbecSDK.so.1.10.37",
    "libob_usb.so",
    "liblive555.so",
    "libdepthengine.so.2.0",
];

fn main() {
    println!("cargo:rerun-if-env-changed=ORBBEC_V1_SDK_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    let target = env::var("TARGET").unwrap();
    let (metadata, machine) = match target.as_str() {
        "x86_64-unknown-linux-gnu" => ("DEP_ORBBEC_V1_BIN_X86_64_ROOT", 62),
        "aarch64-unknown-linux-gnu" => ("DEP_ORBBEC_V1_BIN_AARCH64_ROOT", 183),
        _ => panic!("Supported targets: x86_64-unknown-linux-gnu and aarch64-unknown-linux-gnu"),
    };
    let root = PathBuf::from(env::var_os("ORBBEC_V1_SDK_DIR").unwrap_or_else(|| {
        env::var_os(metadata).expect("Bundled SDK dependency metadata missing")
    }))
    .canonicalize()
    .expect("SDK directory missing; ORBBEC_V1_SDK_DIR must contain lib/");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let staged = out.join("sdk");
    let lib = staged.join("lib");
    fs::create_dir_all(&lib).unwrap();
    for name in LIBRARIES {
        let source = root.join("lib").join(name);
        println!("cargo:rerun-if-changed={}", source.display());
        let bytes = fs::read(&source).unwrap_or_else(|e| panic!("{}: {e}", source.display()));
        assert!(
            bytes.len() > 20 && &bytes[..4] == b"\x7fELF",
            "Invalid SDK ELF library: {name}"
        );
        assert_eq!(bytes[4], 2, "Expected ELF64 SDK: {name}");
        assert_eq!(bytes[5], 1, "Expected little-endian SDK: {name}");
        assert_eq!(
            u16::from_le_bytes([bytes[18], bytes[19]]),
            machine,
            "SDK architecture differs from Cargo TARGET: {name}"
        );
        fs::write(lib.join(name), bytes).unwrap();
    }
    // Archive each binary once; materialize the loader/linker aliases only in OUT_DIR.
    for alias in ["libOrbbecSDK.so", "libOrbbecSDK.so.1.10"] {
        fs::copy(lib.join(LIBRARIES[0]), lib.join(alias)).unwrap();
    }
    fs::copy(
        lib.join("libdepthengine.so.2.0"),
        lib.join("libdepthengine.so"),
    )
    .unwrap();
    let config = root.join("config");
    let config_out = staged.join("config");
    fs::create_dir_all(&config_out).unwrap();
    if config.is_dir() {
        println!("cargo:rerun-if-changed={}", config.display());
        for entry in fs::read_dir(config).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                fs::copy(entry.path(), config_out.join(entry.file_name())).unwrap();
            }
        }
    }
    fs::copy(format!("bindings/{target}.rs"), out.join("bindings.rs")).unwrap();
    println!("cargo:rerun-if-changed=bindings/{target}.rs");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=dylib=OrbbecSDK");
    println!(
        "cargo:rustc-env=ORBBEC_V1_RESOLVED_SDK_DIR={}",
        staged.display()
    );
    println!("cargo:root={}", staged.display());
}
