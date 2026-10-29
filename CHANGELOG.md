# Changelog

## Unreleased

- BREAKING: Wrap lib_init FFI types in native Rust wrappers (UccThreadMode, UccSyncType); remove UccLibParams::inner_mut(); gate UccLibConfig::handle() and UccLib::handle() to pub(crate); UccLibAttrs getters now return native types
- Add safe status helpers (raw_status, is_complete, is_in_progress, check) to UccCollective and UccCollectiveRequest; mark request() as pub(crate)
- Fix CollectiveBuilder allgather truncation: add with_dst_count() to express asymmetric src/dst counts
- Migration: Replace ucc_thread_mode_t_UCC_THREAD_SINGLE with UccThreadMode::Single, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES with UccSyncType::Collectives

## 0.1.0

- Portable `build.rs` (`UCC_PREFIX` / include+lib env)
- `CollectiveBuilder` buffer API uses borrowed slices + lifetime (not raw `usize` mem handles)
- Example: `lib_init_version`
- Fixed optional `ucx-sys` path → `../ucx-rs`
