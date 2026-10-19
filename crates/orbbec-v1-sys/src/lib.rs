//! Raw, unsafe bindings to Orbbec SDK **v1.10.37**. See the workspace README.
#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// SDK staging directory containing `lib/` and `config/` for this build's target.
///
/// Cargo arranges the loader path for `cargo run` and `cargo test`. For deployment,
/// copy these libraries and configuration alongside the executable and configure
/// its loader path. This build-time path is not a portable installation location.
pub fn sdk_directory() -> &'static std::path::Path {
    std::path::Path::new(env!("ORBBEC_V1_RESOLVED_SDK_DIR"))
}
