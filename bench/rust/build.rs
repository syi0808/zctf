fn main() {
    napi_build::setup();
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let fragments = manifest.parent().unwrap().join("schema-fragments");
    println!(
        "cargo:rustc-env=ZCTF_SCHEMA_OUT_DIR={}",
        fragments.display()
    );
}
