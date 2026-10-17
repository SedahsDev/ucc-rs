# Changelog

## Unreleased

### Breaking Changes
- `UccCollectiveRequest` now requires a lifetime parameter `'a` tied to the buffer borrow. All convenience methods on `UccTeam` (`allreduce`, `bcast`, `allgather`, etc.) now return `UccCollectiveRequest<'a>`.
- `UccCollective` also now has a lifetime parameter `'a`.
- `CollectiveBuilder::init` and `CollectiveBuilder::init_and_post` now return `Result<UccCollective<'a>, UccStatus>`.

### Fixes
- `UccTeam::drop` now properly handles `UCC_INPROGRESS` from `ucc_team_destroy` by looping with context progress.

- Fix CollectiveBuilder allgather truncation: add with_dst_count() to express asymmetric src/dst counts

## 0.1.0

- Portable `build.rs` (`UCC_PREFIX` / include+lib env)
- `CollectiveBuilder` buffer API uses borrowed slices + lifetime (not raw `usize` mem handles)
- Example: `lib_init_version`
- Fixed optional `ucx-sys` path → `../ucx-rs`
