fn main() {
    #[cfg(feature = "c_api")]
    generate_headers();
}

#[cfg(feature = "c_api")]
fn generate_headers() {
    use std::{env, path::PathBuf};

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let Ok(include_dir) = env::var("COSMOSCX_INCLUDE_DIR") else {
        // Skip generating headers if COSMOSCX_INCLUDE_DIR isn't set.
        return;
    };

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(PathBuf::from(include_dir).join("cosmoscx.h"));
}
