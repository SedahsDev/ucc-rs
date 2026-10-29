# Changelog

## Unreleased

- Add safe status helpers (raw_status, is_complete, is_in_progress, check) to UccCollective and UccCollectiveRequest; mark request() as pub(crate)
- Fix CollectiveBuilder allgather truncation: add with_dst_count() to express asymmetric src/dst counts
- fix(#17): correct UCC status classification in collective `test()` and the status helpers (`UCC_INPROGRESS`/`UCC_OPERATION_INITIALIZED` = in progress, negative = error, `UCC_ERR_NO_MESSAGE` no longer special-cased) and finalize completed-or-errored requests on drop instead of leaking them

## 0.1.0

- Portable `build.rs` (`UCC_PREFIX` / include+lib env)
- `CollectiveBuilder` buffer API uses borrowed slices + lifetime (not raw `usize` mem handles)
- Example: `lib_init_version`
- Fixed optional `ucx-sys` path → `../ucx-rs`
