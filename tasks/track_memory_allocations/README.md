# track_memory_allocations

Tracks the number of heap allocations made by
[`rolldown_sourcemap`](../../crates/rolldown_sourcemap)'s per-chunk machinery —
`SourceJoiner::join` (chunk assembly + sourcemap merge) and `collapse_sourcemaps`
(the minify / transform chains) — for a fixed set of scenarios, and records the
counts in a committed snapshot ([`allocs.snap`](./allocs.snap)).

CI re-runs the tool and fails if the snapshot changes, so any allocation
regression (or improvement) on this codegen hot path shows up as a reviewable
diff instead of silently slipping in.

Scenarios are driven by real sourcemaps produced by oxc codegen, generated
outside the measured window so each count reflects only the `rolldown_sourcemap`
operation.

## Usage

```bash
just allocs      # or: cargo allocs
```

This regenerates `allocs.snap`. Commit the result if the change is expected
(an intentional improvement), otherwise investigate the regression.

## Notes

- **Counts, not bytes.** The allocation count is stable across platforms and
  closely tracks memory pressure, whereas byte totals vary between
  allocators/platforms (same rationale as oxc's tracker). Allocations are routed
  through a fixed allocator (MiMalloc) for the same reason.
- **`Allocs` vs `Reallocs`:** `Allocs` counts fresh allocations; `Reallocs`
  counts in-place grows (`realloc`). Pre-sizing a `Vec`/`String` moves counts out
  of `Reallocs` — an improvement, not a regression.
- **Measured window:** inputs (the joiner / the map chain) are built up front, so
  each count reflects only `join` / `collapse_sourcemaps`. See `measure` in
  `src/main.rs`.
- **Canonical platform:** the snapshot is regenerated and checked on the Linux CI
  runner, which is authoritative. Counts are platform-independent by design, so a
  local `just allocs` should match; if it ever differs on your OS/arch, trust CI.
- **Dependency bumps:** the counts depend on `rolldown_sourcemap` (and its
  transitive `oxc` / `oxc_sourcemap`) internals, so a bump that changes allocation
  behaviour will legitimately move the snapshot — run `just allocs` and review the
  delta as expected churn.
