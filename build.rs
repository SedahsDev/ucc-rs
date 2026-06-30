use std::env;
use std::path::PathBuf;

fn main() {
    let ucc_prefix = env::var("UCC_PREFIX").unwrap_or_else(|_| "/home/bzf/.local/ucc".to_string());
    let ucc_include = PathBuf::from(&ucc_prefix).join("include");
    let ucc_lib = PathBuf::from(&ucc_prefix).join("lib");

    println!("cargo:rustc-link-search={}", ucc_lib.display());
    println!("cargo:rustc-link-lib=ucc");
    println!("cargo:rerun-if-env-changed=UCC_PREFIX");
    println!("cargo:rerun-if-changed=wrapper.h");

    let src_bindings = PathBuf::from("src").join("bindings.rs");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");

    // Try to generate bindings if UCC headers are available; fall back to pre-generated src/bindings.rs
    if ucc_include.exists() && std::fs::exists(ucc_include.join("ucc/api/ucc.h")).unwrap_or(false) {
        // UCC headers available — generate fresh bindings
        let bindings = bindgen::Builder::default()
            .header("wrapper.h")
            .clang_arg(format!("-I{}", ucc_include.display()))
            .constified_enum(".+")
            .allowlist_type("ucc_.*|FILE|size_t|uint.*|int.*|c_.*|va_list")
            .allowlist_function("ucc_.*")
            .allowlist_var("UCC_.*|ucc_.*")
            .layout_tests(false)
            .derive_copy(true)
            .derive_debug(true)
            .derive_default(false)
            .size_t_is_usize(true)
            .raw_line("#![allow(non_upper_case_globals)]")
            .raw_line("#![allow(non_camel_case_types)]")
            .raw_line("#![allow(non_snake_case)]")
            .raw_line("#![allow(dead_code)]")
            .raw_line("#![allow(clippy::all)]")
            .raw_line("#![allow(unused_unsafe)]")
            .raw_line("#![allow(unnecessary_transmutes)]")
            .generate()
            .expect("Unable to generate UCC bindings");

        bindings
            .write_to_file(&out_path)
            .expect("Failed to write bindings to OUT_DIR");

        // Update src/bindings.rs with fresh bindings
        std::fs::copy(&out_path, &src_bindings).expect("Failed to copy bindings to src/");
        println!("cargo:rerun-if-changed={}", src_bindings.display());
    } else {
        // No UCC headers — use pre-generated src/bindings.rs as offline fallback
        if src_bindings.exists() {
            println!("cargo:warning=UCC headers not found, using pre-generated offline bindings");
            std::fs::copy(&src_bindings, &out_path)
                .expect("Failed to copy pre-generated bindings to OUT_DIR");
        } else {
            panic!("UCC headers not found and no pre-generated src/bindings.rs available. Install UCC or provide offline bindings.");
        }
    }
}
