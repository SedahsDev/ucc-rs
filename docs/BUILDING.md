# Building ucc-rs

## Prerequisites

- Rust stable and Cargo
- `libclang-dev` (required by bindgen)
- `libucx-dev` (required by UCC at build/runtime)

Install the distribution packages for your operating system, or build UCX and UCC from source.

## UCC installation

Build and install UCC from the upstream repository:

```bash
git clone https://github.com/openucx/ucc.git
cd ucc
./autogen.sh
./configure --prefix="$HOME/.local/ucc"
make -j"$(nproc)"
make install
```

Alternatively, install UCC and UCX using system packages when your distribution provides them. Ensure the installed versions are compatible.

## Paths and runtime environment

Set `UCC_PREFIX` to an installation prefix, or set both `UCC_INCLUDE_DIR` and `UCC_LIB_DIR` explicitly. `UCX_PREFIX` may be needed for UCC's UCX transport at runtime:

```bash
export UCC_PREFIX="$HOME/.local/ucc"
export UCX_PREFIX="$HOME/.local/ucx"
export LD_LIBRARY_PATH="$UCC_PREFIX/lib:$UCX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
cargo build
```

The build script also searches `/usr`, `/usr/local`, and `/opt/ucc` when no explicit UCC paths are provided.

## Offline and pre-generated bindings

To build without running bindgen, use the committed bindings:

```bash
UCC_USE_PREGENERATED_BINDINGS=1 cargo build
```

When intentionally refreshing those bindings after a UCC header change, set `UCC_UPDATE_COMMITTED_BINDINGS=1`. Review generated changes carefully and commit them only when intentional.

## Tests

Run the normal suite with the same UCC environment used for building:

```bash
cargo test
```

Multi-rank DVM integration tests are ignored by default. Run them explicitly after installing/configuring a DVM such as PRRTE:

```bash
cargo test -- --ignored
```
