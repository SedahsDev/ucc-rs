//! Minimal UCC example: print version and initialize the library.
//!
//! ```text
//! export UCC_PREFIX=/path/to/ucc
//! cargo run --example lib_init_version
//! ```

use ucc::lib_init::{ucc_version, ucc_version_string, UccLib};

fn main() {
    let (maj, min, rel) = ucc_version();
    println!(
        "UCC version {}.{}.{} ({})",
        maj,
        min,
        rel,
        ucc_version_string()
    );

    let lib =
        UccLib::init().expect("UccLib::init failed — is libucc installed and UCC_PREFIX set?");
    println!("UCC library initialized (handle non-null)");
    drop(lib);
    println!("UCC library finalized");
}
