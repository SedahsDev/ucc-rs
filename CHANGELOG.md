# Changelog

## Unreleased

### Breaking Changes
- `UccExecutionEngine::handle()`, `UccEvent::as_ptr()`, `UccCollective::request()`, `UccCollectiveRequest::request()`, `UccCollectiveType::as_raw()`, `DataType::as_raw()`, `UccReductionOp::as_raw()`, and `UccTeam::handle()` are now `pub(crate)` — raw access is internal only.
- `UccExecutionEngine::set_event_raw()` and `ack_event_raw()` are now `pub(crate)` — use `set_event()` and `ack_event()` with native `UccEvent`.
- `UccCollective::triggered_post()` now takes `&UccExecutionEngine` and `&UccEvent` instead of raw handles and pointers.
- `UccTeam::get_attr()` now returns `UccTeamAttrs` instead of raw `ucc_team_attr`.
- `UccTeamParams::inner_mut()` is removed — use `with_*()` setters instead.
- `UccPostOrdering` enum replaces raw `ucc_post_ordering_t` values in `UccTeamParams::with_ordering()`.
- `UccOobColl` wrapper replaces raw `ucc_oob_coll_t` in `UccTeamParams::with_oob()`.

### Migration
- Replace `ee.handle()` with direct use of the `UccExecutionEngine` instance.
- Replace `event.as_ptr()` with `&event` in `triggered_post()`.
- Replace `params.inner_mut().ordering = value` with `params.with_ordering(UccPostOrdering::Ordered)`.
- Replace `params.inner_mut().oob = value` with `params.with_oob(UccOobColl::new(raw_oob))`.
- Access team attributes through `UccTeamAttrs` typed getters: `attrs.ordering()`, `attrs.outstanding_colls()`, etc.

### Breaking Changes
- `UccContextType` is the native enum replacing `ucc_context_type_t` in `UccContextParams::with_type()` and `UccContextAttrs::context_type()` (#36).
- `UccContextParams` no longer exposes `inner_mut()`; use typed setters instead. `with_oob()` takes the native `UccOobColl` wrapper instead of raw `ucc_context_oob_coll_t` (#36).
- `UccContextAttrs` exposes native attribute values through typed getters, including `context_type()` (#36).
- `UccContextConfig::handle()` is now `pub(crate)`; the `bindings` module is now `pub(crate)`, so bindgen types are no longer reachable from downstream crates (#36).
- `UccExecutionEngineParams` no longer exposes `inner_mut()`; use `with_type(UccEeType)` and `with_context(ptr, size)` instead of modifying raw `ucc_ee_params` fields (#32).
- `UccEeType` is the native enum replacing `ucc_ee_type_t` (#32).
- `UccEvent` replaces raw `ucc_ev_t` in public signatures; `as_ptr()`, `as_raw()`, and `from_raw()` are now `pub(crate)` (#32).
- `UccExecutionEngine::{handle, as_raw}()` and the raw-event setters `set_event_raw`/`ack_event_raw` are now `pub(crate)`, so raw execution-engine handles and events no longer appear in public signatures (#32).
- Wrap collective.rs FFI types: `UccEvent` and `UccExecutionEngine` replace raw `ucc_ev_t`/`ucc_ee_h` in `Collective::triggered_post()` signatures; `as_raw()` on `UccCollectiveType`, `UccReductionOp`, `DataType` is `pub(crate)`; `request()` on `UccCollective`/`UccCollectiveRequest` is `pub(crate)` (#38)
- `UccPostOrdering` is the native enum replacing `ucc_post_ordering_t` in public signatures.
- `UccOobColl` is the native wrapper replacing raw `ucc_oob_coll_t` and its context alias `ucc_context_oob_coll_t` in public signatures.
- `UccTeamParams` replaces raw team parameters in public signatures; `inner_mut()` is removed in favor of typed setters.
- `UccTeamAttrs` replaces raw team attributes returned by `UccTeam::get_attr()` and provides typed getters.
- `UccTeam::handle()` is now `pub(crate)`.
- `UccThreadMode` is the native enum replacing `ucc_thread_mode_t` in public signatures (#30).
- `UccSyncType` is the native enum replacing `ucc_coll_sync_type_t` in public signatures (#30).
- `UccLibParams` no longer exposes `inner_mut()`; use typed setters instead (#30).
- `UccLibAttrs` exposes native attribute values through typed getters instead of bindgen types (#30).
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

### Migration
- Replace `UccLibParams::inner_mut()`, `UccContextParams::inner_mut()`, and `UccTeamParams::inner_mut()` field assignments with the corresponding `with_*()` setters, passing native enums and wrappers rather than bindgen values
- Read library, context, and team attributes through `UccLibAttrs`, `UccContextAttrs`, and `UccTeamAttrs` typed getters; `UccTeam::get_attr()` now returns `UccTeamAttrs`
- Replace `ucc_thread_mode_t_UCC_THREAD_SINGLE` with `UccThreadMode::Single` and `ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES` with `UccSyncType::Collectives`
- Replace raw `ucc_post_ordering_t` values with `UccPostOrdering::Unordered`, `UccPostOrdering::Ordered`, or `UccPostOrdering::Relaxed`
- Replace `triggered_post(ee_handle, event_ptr)` with `triggered_post(&ee, &event)`, using `UccExecutionEngine` and `UccEvent` rather than raw handles or event pointers
- Callers of resource `handle()`, collective `request()`, or enum `as_raw()` methods must use the native wrapper API instead; these raw accessors are now internal
- Replace `params.inner_mut().ee_type = ucc_ee_type_UCC_EE_CPU_THREAD` with `params.with_type(UccEeType::CpuThread)`
- Replace `params.inner_mut().ee_context = ...` with `params.with_context(ptr, size)`
- Raw execution engine handle access via `handle()` is now internal; use EE methods directly
- Replace `ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE` with `UccContextType::Exclusive` and `ucc_context_type_t_UCC_CONTEXT_SHARED` with `UccContextType::Shared`
- For context/team OOB callbacks, construct the native `UccOobColl` (from `team.rs`) instead of the raw `ucc_context_oob_coll_t` / `ucc_oob_coll_t` — they are the same C type
- Bindgen types are no longer re-exported: `ucc::bindings` is `pub(crate)`, so downstream crates must use the native wrappers

## 0.1.0

- Portable `build.rs` (`UCC_PREFIX` / include+lib env)
- `CollectiveBuilder` buffer API uses borrowed slices + lifetime (not raw `usize` mem handles)
- Example: `lib_init_version`
- Fixed optional `ucx-sys` path → `../ucx-rs`
