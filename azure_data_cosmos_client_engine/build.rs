use std::{env, path::PathBuf};

fn main() {
    #[cfg(feature = "c_api")]
    generate_headers();
}

#[cfg(feature = "c_api")]
fn generate_headers() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let artifacts_dir = {
        let dir = env::var("ARTIFACTS_DIR").unwrap_or_else(|_| crate_dir.clone());
        PathBuf::from(dir)
    };

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(artifacts_dir.join("cosmos_client_engine.h"));
}
