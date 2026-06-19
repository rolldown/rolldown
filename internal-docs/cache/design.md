# Cache — Design & Open Questions

> The cache mechanics — inventory, `ScanStageCache`, the module-identity model, the merge path: see [implementation.md](./implementation.md).

## Summary

Rolldown has several distinct cache mechanisms. The architecturally central
one is **`ScanStageCache`** — the bundler-level snapshot of the parsed module
graph that makes incremental builds and HMR possible. The others are
within-build memoization, plugin scratch state, and a JS-side store.

This doc inventories every cache, then details `ScanStageCache`: its data, the
module-identity model it depends on (`ModuleId` / `ModuleIdx` /
`module_id_to_idx`), how `ScanStageCache::merge` splices a partial scan into the
snapshot, and the complete list of readers and writers.

All file/line references are against the working tree at the time of writing
and will drift; treat them as starting points.

## Cache integrity on a failed build

A build mutates `ScanStageCache` through several non-atomic "tear → repair"
steps; an early `?` return between a tear and its repair can leave the cache
broken for the next build. The invariant, the three torn windows
(ownership / scoping / defer-sync), and the unconditional-repair rule are
documented in [bundler-data-lifecycle.md](../bundler-data-lifecycle/implementation.md) ("Cache integrity on a
failed build"). The three fix sites — `with_cached_bundle`, `bundle_up`'s
ordering of `merge_immutable_fields_for_cache`, and `update_defer_sync_data` —
reference that section.

## Unresolved Questions

- `merge`'s `module_id_to_idx[new_module.id()]` index panics on a missing key
  and is reachable only on internal inconsistency; `Module::idx()` yields the
  same value without a fallible lookup. Whether to switch is a tracked
  follow-up (audit that no caller feeds `merge` a `Module` whose `.idx` was not
  loader-allocated).
- `merge` is a large multi-field mutation with no mid-loop `?`, but a panic
  mid-`merge` (the two surfaces above) would leave the snapshot present but
  internally inconsistent. Restoring presence does not guarantee consistency.

## Related

- [implementation.md](./implementation.md) — the cache implementation (inventory, `ScanStageCache`, identity model)
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) — bundler-level vs
  bundle-level data, `BundleMode`, cache integrity on a failed build.
- [module-id](../module-id/implementation.md) — `ModuleId` design.
- [rust-bundler](../rust-bundler/implementation.md) — `Bundler` struct and build lifecycle.
- [watch-mode](../watch-mode/implementation.md) — watch mode, which drives partial scans.
