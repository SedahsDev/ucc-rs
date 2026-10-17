# ucc

Safe Rust bindings for [UCC](https://github.com/openucx/ucc) (Unified Collective Communication).

## Features

- RAII handles: `UccLib`, `UccContext`, `UccTeam`, collectives, memory map
- Type-safe status (`UccError` / `UccStatus`), datatypes, reduction ops
- Team convenience methods: `barrier`, `allreduce`, `allgather`, `bcast`, `reduce`
- `CollectiveBuilder` with borrow-checked buffer slices
- Optional `ucx-integration` feature (path dep on `ucx-sys`)

## Build

```bash
export UCC_PREFIX=/path/to/ucc
export UCX_PREFIX=/path/to/ucx   # often needed at runtime for UCC TLs
export LD_LIBRARY_PATH=$UCC_PREFIX/lib:$UCX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
cargo build
cargo test
cargo run --example lib_init_version
```

Also: `UCC_INCLUDE_DIR` + `UCC_LIB_DIR`. Fallbacks: `/usr`, `/usr/local`, `/opt/ucc` (not home paths).

See [`docs/BUILDING.md`](docs/BUILDING.md).

## Lifecycle

```
UccLib::init()
  └─ UccContext::new(lib)
      └─ UccTeam::new(ctx)
          └─ team.barrier() / allreduce(...) / CollectiveBuilder ...
```

## Minimal examples

Library init + version (always works if libucc is installed):

```rust
use ucc::lib_init::{ucc_version_string, UccLib};

fn main() {
    println!("{}", ucc_version_string());
    let _lib = UccLib::init().expect("ucc init");
}
```

Single-rank team barrier sketch (multi-rank collectives usually need OOB + DVM):

```rust
use ucc::context::UccContext;
use ucc::lib_init::UccLib;
use ucc::team::UccTeam;

fn main() {
    let lib = UccLib::init().unwrap();
    let ctx = UccContext::new(lib).unwrap();
    let team = UccTeam::new(ctx.clone()).unwrap();
    // Multi-process barrier often needs prterun + OOB; see REVIEW.md
    let _ = team;
}
```

## Safety notes

- Keep collective buffers alive until requests complete.
- `CollectiveBuilder` ties buffer slices to a lifetime and takes raw host pointers for UCC.

## Polling for collective completion

Safe helper methods prevent direct access to raw FFI status fields.

```rust
use ucc::collective::{CollectiveBuilder, UccCollectiveType};
use ucc::datatype::DataType;
use ucc::lib_init::UccLib;
use ucc::reduction::UccReductionOp;
use ucc::team::UccTeam;

fn example(team: &UccTeam) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = [0u8; 256];
    let coll = CollectiveBuilder::new(UccCollectiveType::Allreduce)
        .with_inplace(&mut buf)
        .with_count(256)
        .with_dtype(DataType::Uchar)
        .with_reduction_op(UccReductionOp::Sum)
        .init_and_post(team)?;

    // Poll for completion using safe helpers
    loop {
        if coll.is_complete() {
            break;
        }
        if coll.is_in_progress() {
            // Still running, continue polling
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        // Check for errors
        if let Err(e) = coll.check() {
            return Err(e.into());
        }
    }

    Ok(())
}
```

Note: The `request()` handle is `pub(crate)` to avoid exposing bindgen types in the public API. Use `is_complete()`, `is_in_progress()`, and `check()` instead of reading `request().status` directly.

## License

BSD-style (see `LICENSE`). See [`REVIEW.md`](./REVIEW.md) for full review.
