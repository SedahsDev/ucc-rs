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

## License

BSD-style (see `LICENSE`). See [`REVIEW.md`](./REVIEW.md) for full review.
