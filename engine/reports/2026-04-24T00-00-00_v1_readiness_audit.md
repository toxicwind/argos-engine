# v1-readiness audit + first wave of fixes — 2026-04-24

Branch: `prod/v1-readiness` off `perf/sync-engine`.

## Audit corrections (false positives from the first agent pass)

The initial production-readiness audit flagged 25 `panic!()` sites
across `argos-query/parser.rs`, `argos-query/rewriter.rs`,
`argos-compress/field_codec.rs`, `argos-storage/wal.rs`,
`argos-storage/doc_values.rs`, and `argos-vector/quantizer.rs` as a
P0 production DOS vector.

Verified by re-grep with `#[cfg(test)]` boundary check: **all 25
panics are inside `mod tests` blocks**.  Zero are reachable in
production.  Specifically:

| File | Test boundary | Panics flagged |
|---|---|---|
| argos-query/parser.rs | `#[cfg(test)]` at line 2254 | 10 panics, all at line 2306+ |
| argos-query/rewriter.rs | `#[cfg(test)]` at line 334 | 4 panics, all at line 408+ |
| argos-compress/field_codec.rs | `#[cfg(test)]` at line 998 | 6 panics, all at line 1105+ |
| argos-storage/wal.rs | `#[cfg(test)]` at line 969 | 1 panic at line 1000 |
| argos-storage/doc_values.rs | `#[cfg(test)]` at line 664 | 1 panic at line 760 |
| argos-vector/quantizer.rs | `#[cfg(test)]` at line 423 | 2 panics at line 537–8 |

The `/metrics` endpoint was also flagged as missing.  Verified at
`engine/crates/argos-api/src/router.rs:93` (`route("/v1/metrics",
get(native::metrics))`), handler at
`engine/crates/argos-api/src/native.rs:513`.  Already wired.

**Real production blockers** that remain:
1. ES-compat work lives on `fix/es-compat-phase0` in the sibling
   repo `/home/claude/ai/argos-es-compat-work/` — 98.12 % YAML
   pass, 424 commits ahead, **not yet merged into core**.
2. Cluster Raft log is in-memory only (`argos-cluster/raft_log.rs`).
   Affects multi-node only; single-node deploys unaffected.
3. Perf backlog (see below).

## Commits landed on this branch

1. `f0221a4` — perf(argos-engine): condvar `wait_for_drain`
   replaces 5 ms × 10 thread-sleep loop in soft back-pressure.
   Was already pending in the working tree on `perf/sync-engine`;
   committed onto the new branch with full body.
2. `723a8b1` — perf(argos-storage): VersionEntry.segment_id is
   now `Arc<str>`; per-batch hoist of `Arc::from(IN_MEMORY_SEGMENT_ID)`
   eliminates per-doc String allocation in `wal_append_batch_raw`,
   `apply_flush`, `version_map_rebuild`, and post-merge fixup.

## Perf backlog (next candidates, file:line cited)

| Rank | Candidate | Estimated win | Risk | Files |
|---|---|---|---|---|
| 1 | Per-shard flush thresholds (replace global soft_block) | 10–15 % ingest | Med | engine/index.rs:1093–1115, 1237–1241 |
| 2 | Split FTS memtable write lock | 15–25 % p99, 1–2 % throughput | Med | engine/index.rs:1037–1172, engine/memtable.rs |
| 3 | Lock-free version_map bulk update on merge | 3–5 % flush | Low | engine/index.rs:1943 |
| 4 | Preallocate format!() escape buffer | <1 % ingest | Low | storage/index_store.rs:1295 |
| 5 | Segment write pipelining (Phase 2 build || Phase 2 fsync) | 8–12 % flush | High | engine/index.rs:4932 |
| 6 | ShardedFtsMemtable (scaffold exists) | 20–30 % ingest, 10–15 % query | High | engine/memtable.rs:200–250 |

## ES-compat merge plan

Recommended strategy: rebase `prod/v1-readiness` onto
`fix/es-compat-phase0`.  Only 2 perf commits to replay.  Conflicts
expected in `engine/index.rs`, `engine/memtable.rs`, `Cargo.lock`.

Sibling-side new files (no conflict, pure adds):
- `engine/crates/argos-engine/src/aggs.rs` (+9 797 lines)
- `engine/crates/argos-engine/src/painless.rs` (+1 069 lines, NEW)
- `engine/crates/argos-engine/src/bulk.rs` (+1 044 lines)
- `engine/crates/argos-api/src/es_compat.rs` (+11 966 lines)

Test verification after merge: `cargo build --release` (~4 min
cold), `cargo test` (~2 min), `es-yaml-runner` (~2.5 min).  Expect
~1304/1329 pass.
