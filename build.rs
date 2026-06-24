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

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", ucc_include.display()))
        // Generate constants for ALL enums (anonymous and named)
        // This turns enum variants into pub const UCC_* values
        .constified_enum(".+")
        // Allow UCC types, functions, and generated constants
        .allowlist_type("ucc_.*|FILE|size_t|uint.*|int.*|c_.*|va_list")
        .allowlist_function("ucc_.*")
        .allowlist_var("UCC_.*|ucc_.*")
        // Generate proper Rust types
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

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    let src_path = PathBuf::from("src").join("bindings.rs");

    bindings
        .write_to_file(&out_path)
        .expect("Failed to write bindings to OUT_DIR");

    std::fs::copy(&out_path, &src_path).expect("Failed to copy bindings to src/");
    println!("cargo:rerun-if-changed={}", src_path.display());
}
