# Changelog

## Unreleased

### Breaking Changes
- `UccCollectiveRequest` now requires a lifetime parameter `'a` tied to the buffer borrow. All convenience methods on `UccTeam` (`allreduce`, `bcast`, `allgather`, etc.) now return `UccCollectiveRequest<'a>`.
- `UccCollective` also now has a lifetime parameter `'a`.
- `CollectiveBuilder::init` and `CollectiveBuilder::init_and_post` now return `Result<UccCollective<'a>, UccStatus>`.
- `as_raw()` on `UccCollectiveType`, `UccReductionOp`, and `DataType` is now `pub(crate)`; bindgen types (`ucc_coll_type_t`, `ucc_reduction_op_t`, `ucc_datatype_t`, `ucc_coll_req_h`) no longer appear in public signatures.
- `handle()` on `UccLibConfig`, `UccLib`, `UccContext`, `UccTeam`, and `UccMemHandle` is now `pub(crate)` (`UccMemHandle::handle` is test-only); the bindgen handle types (`ucc_lib_h`, `ucc_lib_config_h`, `ucc_context_h`, `ucc_team_h`, `ucc_mem_map_mem_h`) no longer appear in public signatures.

### Features
- `UccLib::finalize(&mut self) -> Result<(), UccStatus>`: explicit, fallible teardown of the library handle (#25). Rejected with an error while other `UccLib` clones exist or after a prior finalize; `Drop` still auto-finalizes when `finalize()` was not called.

### Fixes
- `UccTeam::drop` now properly handles `UCC_INPROGRESS` from `ucc_team_destroy` by looping with context progress.
- Add safe status helpers (raw_status, is_complete, is_in_progress, check) to UccCollective and UccCollectiveRequest; mark request() as pub(crate)
- Fix CollectiveBuilder allgather truncation: add with_dst_count() to express asymmetric src/dst counts
- fix(#17): correct UCC status classification in collective `test()` and the status helpers (`UCC_INPROGRESS`/`UCC_OPERATION_INITIALIZED` = in progress, negative = error, `UCC_ERR_NO_MESSAGE` no longer special-cased) and finalize completed-or-errored requests on drop instead of leaking them

## 0.1.0

- Portable `build.rs` (`UCC_PREFIX` / include+lib env)
- `CollectiveBuilder` buffer API uses borrowed slices + lifetime (not raw `usize` mem handles)
- Example: `lib_init_version`
- Fixed optional `ucx-sys` path → `../ucx-rs`
