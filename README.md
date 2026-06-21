# ucc-rs

Safe Rust bindings for [UCC](https://github.com/openucx/ucc) (Unified Collective Communication).

## Overview

UCC provides native collective operations built on top of UCX. This crate provides:
- Auto-generated FFI bindings (via bindgen)
- Safe Rust wrappers for the full UCC lifecycle

## UCC API Lifecycle

```
ucc_init_version() → UccLib
    └─ ucc_context_create() → UccContext
        └─ ucc_team_create_post() + ucc_team_create_test() → UccTeam
            ├─ ucc_collective_init() + ucc_collective_post() → UccCollRequest
            │   └─ ucc_collective_finalize() -- cleanup
            └─ ucc_ee_create() → UccExecutionEngine (for triggered collectives)
```

## Building

Requires UCC library installed. Set `UCC_PREFIX` or use default `/home/bzf/.local/ucc/`.

```bash
cargo build
```

## Modules

- `status` - UCC status codes and error handling
- `lib_init` - Library initialization (`UccLib`)
- `context` - Context creation (`UccContext`)
- `team` - Team creation and management (`UccTeam`)
- `collective` - Collective operations (barrier, bcast, allreduce, etc.)
- `event_engine` - Execution Engine for triggered collectives
- `memory` - Memory mapping for UCC operations

## Non-blocking Design

All collectives return a `UccCollRequest` that must be polled via `test()` or `wait()`.
This follows the user preference for non-blocking I/O patterns throughout.
