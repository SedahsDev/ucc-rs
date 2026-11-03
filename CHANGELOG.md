# Changelog

## Unreleased

### Breaking Changes
- Wrap collective.rs FFI types: `UccEvent` and `UccExecutionEngine` replace raw `ucc_ev_t`/`ucc_ee_h` in `Collective::triggered_post()` signatures; `as_raw()` on `UccCollectiveType`, `UccReductionOp`, `DataType` is `pub(crate)`; `request()` on `UccCollective`/`UccCollectiveRequest` is `pub(crate)` (#38)
- Wrap team FFI types in native Rust wrappers: `UccPostOrdering`/`UccOobColl`/`UccTeamParams`/`UccTeamAttrs` replace raw bindgen types in public signatures; `UccTeamParams::inner_mut()` removed (typed setters instead); `UccTeam::get_attr()` returns `UccTeamAttrs` with typed getters; `UccTeam::handle()` is now `pub(crate)`
- Wrap lib_init FFI types in native Rust wrappers: `UccThreadMode`/`UccSyncType` enums replace `ucc_thread_mode_t`/`ucc_coll_sync_type_t` in public signatures; `UccLibParams::inner_mut()` removed (typed setters instead); `UccLibAttrs` getters return native types (#30)
- `UccCollectiveRequest` now requires a lifetime parameter `'a` tied to the buffer borrow. All convenience methods on `UccTeam` (`allreduce`, `bcast`, `allgather`, etc.) now return `UccCollectiveRequest<'a>`.
- `UccCollective` also now has a lifetime parameter `'a`.
- `CollectiveBuilder::init` and `CollectiveBuilder::init_and_post` now return `Result<UccCollective<'a>, UccStatus>`.
- `as_raw()` on `UccCollectiveType`, `UccReductionOp`, and `DataType` is now `pub(crate)`; bindgen types (`ucc_coll_type_t`, `ucc_reduction_op_t`, `ucc_datatype_t`, `ucc_coll_req_h`) no longer appear in public signatures.
- `handle()` on `UccLibConfig`, `UccLib`, `UccContext`, `UccTeam`, and `UccMemHandle` is now `pub(crate)` (`UccMemHandle::handle` is test-only); the bindgen handle types (`ucc_lib_h`, `ucc_lib_config_h`, `ucc_context_h`, `ucc_team_h`, `ucc_mem_map_mem_h`) no longer appear in public signatures.

### Features
- `UccLib::finalize(&mut self) -> Result<(), UccStatus>`: explicit, fallible teardown of the library handle (#25). Rejected with an error while other `UccLib` clones exist or after a prior finalize; `Drop` still auto-finalizes when `finalize()` was not called.

### Documentation
- Document the buffer-lifetime requirement on the `UccTeam` convenience collectives (`allreduce`, `allgather`, `bcast`, `reduce`, `gather`, `scatter`, `alltoall`, `reduce_scatter`) and on `CollectiveBuilder`: the buffer slices must outlive the returned request, which the `'a` lifetime enforces (#48)

### Tests
- Add edge-case tests for allgather_counts: `allgather_counts_zero_send_elems` and `allgather_counts_single_rank` (#49)

### Fixes
- `UccTeam::drop` now properly handles `UCC_INPROGRESS` from `ucc_team_destroy` by looping with context progress.
- Add safe status helpers (raw_status, is_complete, is_in_progress, check) to UccCollective and UccCollectiveRequest; mark request() as pub(crate)
- Fix `UccTeam::drop` to re-drive `ucc_team_destroy` on `UCC_INPROGRESS`; add fallible `UccTeam::destroy()` for explicit teardown
- Fix CollectiveBuilder allgather truncation: add with_dst_count() to express asymmetric src/dst counts
- fix(#17): correct UCC status classification in collective `test()` and the status helpers (`UCC_INPROGRESS`/`UCC_OPERATION_INITIALIZED` = in progress, negative = error, `UCC_ERR_NO_MESSAGE` no longer special-cased) and finalize completed-or-errored requests on drop instead of leaking them
- Migration: Replace `ucc_thread_mode_t_UCC_THREAD_SINGLE` with `UccThreadMode::Single`, `ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES` with `UccSyncType::Collectives`
- Migration: Replace `ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES` with `UccSyncType::Collectives`, `ucc_post_ordering_t` with `UccPostOrdering::Unordered`/`Ordered`/`Relaxed`, use `params.with_*()` setters instead of `inner_mut().field = value`
- Migration: Replace `triggered_post(ee_handle, event_ptr)` with `triggered_post(&ee, &event)` using native `UccExecutionEngine` and `UccEvent` types

## 0.1.0

- Portable `build.rs` (`UCC_PREFIX` / include+lib env)
- `CollectiveBuilder` buffer API uses borrowed slices + lifetime (not raw `usize` mem handles)
- Example: `lib_init_version`
- Fixed optional `ucx-sys` path → `../ucx-rs`
