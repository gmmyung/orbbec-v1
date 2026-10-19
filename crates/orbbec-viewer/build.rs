fn main() {
    // Deployed layout: executable next to orbbec-sdk/{lib,config}.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/orbbec-sdk/lib");
}
