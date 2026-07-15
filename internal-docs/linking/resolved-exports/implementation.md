# Resolved exports

## Summary

`CollectResolvedExportsPass` computes each normal module's direct and transitive export-star view after lazy-export normalization has finalized every local export identity. The pass reads only `ModuleTable`, returns one owned dense `ResolvedExportsDraft`, and never reads or mutates `LinkStage`, linking metadata, the symbol database, wrapper state, side effects, or options.

The driver immediately consumes the artifact after the pass. Each normal-module map moves into the unchanged `LinkingMetadata::resolved_exports` compatibility field without cloning; an external slot remains `None`, while a normal module with no exports remains `Some(empty)`.

Source: `crates/rolldown/src/stages/link_stage/passes/collect_resolved_exports.rs`.

## Pipeline placement

```text
NormalizeLazyExportsPass
  ├─ final wrappers and ModuleTable → DetermineModuleSideEffectsPass → side-effect and representation compatibility projections
  └─ final ModuleTable and local SymbolRef identities → CollectResolvedExportsPass → owned ResolvedExportsDraft → checked no-clone projection → bind_imports_and_exports

DetermineModuleSideEffectsPass and its projections ··· current driver order only ···> CollectResolvedExportsPass
```

`NormalizeLazyExportsPass` is the last operation that may rebuild a lazy or JSON module's `named_exports` and owner-local `SymbolRef` identities. Collection before that point could retain invalid symbols or omit exports created by normalization. Module-side-effect analysis and resolved-export collection are semantically independent after normalization: the current driver finishes side-effect and representation compatibility projections before invoking resolved-export collection, but `CollectResolvedExportsPass` does not read their artifacts or projected fields. That serial order preserves the statically visible trace, while each resolved-export root already runs in parallel. Driver-level overlap remains a measured follow-up rather than part of this extraction.

The compatibility projection validates all three dense layouts: the artifact, `ModuleTable`, and `LinkingMetadataVec` must have the same length; every normal module must have `Some(map)` and every external module must have `None`. It then moves each map once into metadata. Binding reads those maps to match imports and derive separate sorted and non-ambiguous export facts, but it neither creates nor extends the maps. This projection is temporary: the later Phase 4 binding split keeps `ResolvedExportsDraft` typed through `BindImportsPass`, then `FinalizeResolvedExportsPass` consumes it after the last symbol link and produces the finalized export artifact for the output adapter.

## Pass contract

| Slot            | Type                   | Purpose                                                                                                   |
| --------------- | ---------------------- | --------------------------------------------------------------------------------------------------------- |
| `InputRead<'a>` | `&'a ModuleTable`      | Borrows the final module table after lazy normalization.                                                  |
| `InputOwned`    | `()`                   | No mutable entity table or draft enters the pass.                                                         |
| `OutputRead`    | `()`                   | No link-local sealed fact is minted.                                                                      |
| `OutputOwned`   | `ResolvedExportsDraft` | Owns the maps that must later move into the legacy output; keeping them owned avoids a graph-sized clone. |
| `Error`         | `Infallible`           | The existing link boundary remains infallible.                                                            |

`ResolvedExportsDraft` contains `IndexVec<ModuleIdx, Option<FxHashMap<CompactStr, ResolvedExport>>>`. It is not `Clone`, exposes only its module count and one consuming `into_slots` operation, and has no general mutation API. Physical slot identity is authoritative: the parallel root iterator derives `ModuleIdx` from `0..modules.len()` instead of trusting the embedded `NormalModule::idx` field.

## Exact algorithm

The outer range is an indexed parallel iterator on native targets and the serial compatibility iterator on WASM. Collection into `Vec` preserves physical module order in both implementations, and `IndexVec::from_vec` restores the dense identity type. Every root owns its result map and DFS stack, so roots are independent while each root preserves the legacy serial traversal.

For each physical slot:

1. An external module yields `None`.
2. A normal module first copies every direct `named_exports` entry into a fresh map. The copied `ResolvedExport` keeps the source `SymbolRef` and `came_from_commonjs`; both conflict vectors start empty.
3. A module without ordinary star exports and without `IsCjsReexport` yields that direct map immediately.
4. Otherwise, DFS begins with a fresh path-local stack. A module already on that stack terminates only the current cycle; there is no global visited set, memoized child map, SCC analysis, or fixed point.
5. Each recursive node visits ordinary star targets in physical import-record order, then CJS reexport targets in `cjs_reexport_import_record_ids` order. Unresolved records and external targets are skipped.
6. For each normal target, the algorithm examines that target's direct `named_exports` before recursing. It does not reuse the target's independently collected result, because ancestor-local shadowing differs by path.
7. An ESM-derived `default` is skipped; a CJS-derived `default` is retained. The edge kind and `ExportsKind` do not alter this rule.
8. A candidate is fully shadowed when any normal module on the current DFS path has a direct export with the same name.
9. A new name records the candidate as the primary symbol and preserves its `came_from_commonjs` bit. A later candidate with the same raw `SymbolRef` records nothing. A different symbol appends to `potentially_ambiguous_symbol_refs` when both sources are ESM-derived, or to `cjs_conflicting_symbol_refs` when either source is CJS-derived.
10. Later candidates never replace the primary symbol and never change its `came_from_commonjs` bit. Canonical-symbol comparison remains in binding after symbol linking; this pass deliberately compares raw refs and does not read `SymbolRefDb`.

The order of ordinary records, the separate CJS vector, direct-export hash iteration, DFS recursion, and conflict-vector appends is preserved exactly. These values affect later ambiguity diagnostics and CommonJS fallback behavior, so they are compatibility requirements rather than incidental implementation details.

## Coverage

Focused tests pin:

- dense physical normal, empty-normal, and external slots, including a deliberately mismatched embedded module index;
- ordinary star record order and primary-symbol selection;
- path-local shadowing, shared-dependency revisits, and cycle termination in one diamond-cycle graph;
- ESM `default` exclusion, CJS `default` retention, and CommonJS normal-module traversal;
- raw-identical symbols producing no ambiguity or CJS conflict;
- separate ESM ambiguity and CJS conflict vectors, including both ESM-first and CJS-first provenance;
- ordinary-source ordering before CJS sources, CJS vector ordering, and immutable primary provenance;
- CJS-only traversal at a recursive node;
- external and unresolved ordinary and CJS edges; and
- reading the final module table after upstream normalization.

The fourteen-pass production trace test pins `CollectResolvedExportsPass` after `NormalizeLazyExportsPass` and `DetermineModuleSideEffectsPass`. Broader Rust, Node, Rollup, deterministic digest, WASM, timing, and memory gates remain in the pass-pipeline validation matrix.

## Related

- [Pass-based pipeline implementation](../../pass-based-pipeline/implementation.md)
- [Pass-based pipeline design](../../pass-based-pipeline/design.md)
- [Module side effects](../module-side-effects/implementation.md)
- [Determine module formats](../determine-module-exports-kind/implementation.md)
