use std::env;
use std::path::{Path, PathBuf};

/// Discover UCC include/lib dirs.
/// Order: UCC_PREFIX → UCC_INCLUDE_DIR/UCC_LIB_DIR → common prefixes → /usr
fn discover_ucc() -> (PathBuf, PathBuf) {
    println!("cargo:rerun-if-env-changed=UCC_PREFIX");
    println!("cargo:rerun-if-env-changed=UCC_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=UCC_LIB_DIR");

    if let Ok(prefix) = env::var("UCC_PREFIX") {
        let prefix = PathBuf::from(prefix);
        return (prefix.join("include"), prefix.join("lib"));
    }

    let include = env::var("UCC_INCLUDE_DIR").ok().map(PathBuf::from);
    let lib = env::var("UCC_LIB_DIR").ok().map(PathBuf::from);
    if let (Some(inc), Some(lib)) = (include, lib) {
        return (inc, lib);
    }

    let candidates = ["/usr", "/usr/local", "/opt/ucc"];
    for c in candidates {
        let p = Path::new(c);
        let inc = p.join("include");
        let lib = p.join("lib");
        if inc.join("ucc").join("api").join("ucc.h").exists()
            || inc.join("ucc.h").exists()
            || lib.join("libucc.so").exists()
            || lib.join("libucc.so.1").exists()
        {
            return (inc, lib);
        }
    }

    (PathBuf::from("/usr/include"), PathBuf::from("/usr/lib"))
}

fn main() {
    let (include_dir, lib_dir) = discover_ucc();

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=ucc");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-changed=wrapper.h");

    let src_path = PathBuf::from("src").join("bindings.rs");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        // Generate constants for ALL enums (anonymous and named)
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
        .raw_line("#![allow(unnecessary_transmutes)]");

    // Some installs nest headers under include/ucc/api
    let nested = include_dir.join("ucc").join("api");
    if nested.exists() {
        builder = builder.clang_arg(format!("-I{}", nested.display()));
    }

    match builder.generate() {
        Ok(bindings) => {
            println!("cargo:warning=bindgen succeeded — generating fresh UCC bindings");
            bindings
                .write_to_file(&out_path)
                .expect("Failed to write bindings to OUT_DIR");
            let _ = std::fs::copy(&out_path, &src_path);
        }
        Err(e) => {
            println!(
                "cargo:warning=bindgen failed ({e}) — using pre-generated src/bindings.rs as fallback"
            );
            if src_path.exists() {
                std::fs::copy(&src_path, &out_path)
                    .expect("Failed to copy fallback bindings to OUT_DIR");
            } else {
                panic!(
                    "bindgen failed and no pre-generated src/bindings.rs found.\n\
                     Set UCC_PREFIX (or UCC_INCLUDE_DIR + UCC_LIB_DIR), install libclang-dev,\n\
                     or provide src/bindings.rs."
                );
            }
        }
    }

    println!("cargo:rerun-if-changed={}", src_path.display());
}
