# Bindings Regeneration Contract

## Current State

The committed `src/bindings.rs` is the source of truth until issue #1 is resolved.

## Committed Bindings Are Source of Truth

Until issue #1 is resolved, the committed `src/bindings.rs` is the source of truth. Do not regenerate bindings unless you are fixing issue #1 or the UCC headers have changed.

## When to Rerun Bindgen

Rerun bindings generation when:
- UCC header files change (API additions, modifications)
- UCC version is upgraded
- Binding issues are discovered that require regeneration

## How to Regenerate

Set the following environment variables before running the build:

```bash
export UCC_GENERATE_BINDINGS=1
export UCC_UPDATE_COMMITTED_BINDINGS=1  # optional, updates committed bindings
```

## Version Pinning

Bindings are generated against UCC 1.9.0 headers. This version is pinned to ensure compatibility.

## Validation

Regenerated bindings must pass the same test suite as the committed bindings. Any test failures indicate breaking changes that require review.
