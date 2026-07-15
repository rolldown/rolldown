# Resolved exports

## Summary

Resolved exports now have an explicit draft-to-final lifecycle. `CollectResolvedExportsPass` computes each normal module's direct and transitive export-star view after lazy-export normalization has finalized local export identities. It reads only `ModuleTable` and returns owned `ResolvedExportsDraft` maps containing raw symbol references, provenance, and conflict vectors. `BindImportsPass` borrows that draft while it commits every Link-stage symbol link. `FinalizeResolvedExportsPass` then consumes the draft, reads the linked `SymbolRefDb`, and produces owned `ResolvedExports` with both the unchanged raw maps and a sorted canonical non-ambiguous view.

The final artifact remains typed through `ResolveMemberExpressionsPass`, `CollectEntryExportRootsPass`, and `CreateSyntheticExportStatementsPass`. Only after those readers and N finish does the driver move both maps into `LinkingMetadata` without cloning. An external slot stays `None`, while a normal module with no exports stays `Some(empty)`.

Sources: `crates/rolldown/src/stages/link_stage/passes/collect_resolved_exports.rs` and `crates/rolldown/src/stages/link_stage/passes/finalize_resolved_exports.rs`.

## Pipeline placement

```text
NormalizeLazyExportsPass
  ├─ final wrappers and ModuleTable → DetermineModuleSideEffectsPass → sealed ModuleSideEffects
  └─ final ModuleTable and local SymbolRef identities → CollectResolvedExportsPass → ResolvedExportsDraft

ModuleDependenciesDraft + sealed ModuleExecutionOrders + final ModuleFormats
  + sealed DynamicExports + sealed ModuleSideEffects + ResolvedExportsDraft
  → BindImportsPass
  → final Link-stage SymbolRefDb links
  → FinalizeResolvedExportsPass
  → raw maps + sorted canonical non-ambiguous maps
  → ResolveMemberExpressionsPass
  → CollectEntryExportRootsPass
  → CreateSyntheticExportStatementsPass
  → ReferenceNeededSymbolsPass (does not read resolved exports)
  → checked no-clone compatibility projection
```

`NormalizeLazyExportsPass` is the last operation that may rebuild a lazy or JSON module's `named_exports` and owner-local `SymbolRef` identities. Collection before that point could retain invalid symbols or omit exports created by normalization. `DetermineModuleSideEffectsPass` and collection remain semantically independent after normalization, but their current order is visible in the production pass trace.

The driver may project compact representation facts into legacy fields before collection because remaining legacy code still reads them. It does not end the typed lifetimes needed by binding: `ModuleDependenciesDraft`, sealed `ModuleExecutionOrders`, final `ModuleFormats`, sealed `DynamicExports`, sealed `ModuleSideEffects`, and `ResolvedExportsDraft` all remain available until `BindImportsPass` borrows or consumes them. Execution orders die after B. Formats, dynamic exports, and side effects remain typed through N; the dependency draft moves from B into M. Final `ResolvedExports` remains typed through entry-root collection and synthetic statement creation, then is projected after N.

`BindImportsPass` is the final production code in Link that calls `SymbolRefDb::link`. It is serial and preserves the existing immediate behavior: each named import is recursively matched, its diagnostic, dependency, namespace alias, and symbol link are committed before the next import, and private external binding groups are committed to facade symbols at the end of the same pass. The pass does not yet separate analysis from commit into a pure event plan; that remains future work. `FinalizeResolvedExportsPass` must therefore run after binding, not merely after collection.

Generate has its own later `SymbolRefDb::link` calls during code splitting. They are outside the Link boundary and do not invalidate the finalized Link artifact: Generate already consumes the projected compatibility representation and has separate ownership of the link output.

## Collection pass contract

| Slot            | Type                   | Purpose                                                                                                     |
| --------------- | ---------------------- | ----------------------------------------------------------------------------------------------------------- |
| `InputRead<'a>` | `&'a ModuleTable`      | Borrows the final module table after lazy normalization.                                                    |
| `InputOwned`    | `()`                   | No mutable entity table or draft enters the pass.                                                           |
| `OutputRead`    | `()`                   | No link-local sealed fact is minted.                                                                        |
| `OutputOwned`   | `ResolvedExportsDraft` | Owns raw maps through binding and finalization; keeping them owned avoids a graph-sized compatibility copy. |
| `Error`         | `Infallible`           | The existing link boundary remains infallible.                                                              |

`ResolvedExportsDraft` contains `IndexVec<ModuleIdx, Option<FxHashMap<CompactStr, ResolvedExport>>>`. It is not `Clone`, exposes its module count and a narrow shared lookup for binding, and has one consuming slot conversion used only by finalization. Physical slot identity is authoritative: the parallel root iterator derives `ModuleIdx` from `0..modules.len()` instead of trusting the embedded `NormalModule::idx` field.

## Collection algorithm

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
10. Later candidates never replace the primary symbol and never change its `came_from_commonjs` bit. Canonical-symbol comparison remains in finalization after symbol linking; collection deliberately compares raw refs and does not read `SymbolRefDb`.

The order of ordinary records, the separate CJS vector, direct-export hash iteration, DFS recursion, and conflict-vector appends is preserved exactly. These values affect later ambiguity diagnostics and CommonJS fallback behavior, so they are compatibility requirements rather than incidental implementation details.

## Finalization pass contract

| Slot            | Type                   | Purpose                                                                                                               |
| --------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `InputRead<'a>` | `&'a SymbolRefDb`      | Reads canonical identities after `BindImportsPass` has committed the last Link-stage links.                           |
| `InputOwned`    | `ResolvedExportsDraft` | Consumes the unique raw draft and moves every raw map without cloning.                                                |
| `OutputRead`    | `()`                   | The final maps must later move into the legacy boundary, so they remain on the owned channel.                         |
| `OutputOwned`   | `ResolvedExports`      | Owns one physical slot per module, with the raw map and sorted canonical non-ambiguous map paired for normal modules. |
| `Error`         | `Infallible`           | The link boundary remains infallible.                                                                                 |

Finalization is independent per module and uses the native parallel iterator with the serial WASM compatibility implementation. `ResolvedExportsDraft::into_slots()` moves its `IndexVec` directly into the owning parallel iterator, so the input handoff uses the existing backing allocation and does not copy slot values or raw maps. Collection allocates a new dense per-module slot `Vec` for the finalized artifact because each normal slot changes type from a raw map to `ResolvedExportsForModule`. Every raw map moves once into that wrapper and is never cloned.

For every raw export, finalization compares the primary symbol's canonical ref with every entry in `potentially_ambiguous_symbol_refs`. The name is retained when every ESM ambiguity canonicalizes to the primary and excluded when any remains distinct. `cjs_conflicting_symbol_refs` does not participate in this ESM ambiguity test. The raw `ResolvedExport`, including primary provenance and both conflict vectors, is not rewritten.

The retained `(name, came_from_commonjs)` pairs are sorted and collected into `FxIndexMap<CompactStr, bool>`. `ResolvedExports` provides raw lookup, raw iteration, and canonical-name membership to `ResolveMemberExpressionsPass`; normal-slot shape and ordered canonical export iteration to entry-root collection; and canonical emptiness plus ESM-only canonical iteration to synthetic statement creation. It exposes one consuming slot conversion for the checked compatibility projection.

The projection verifies artifact, module-table, and metadata lengths and the normal-versus-external slot shape. It then moves the raw `FxHashMap` into `resolved_exports` and the sorted `FxIndexMap` into `sorted_and_non_ambiguous_resolved_exports`. This happens only after M, entry-root collection, synthetic statement creation, and N finish, so no clone or early legacy read is needed.

## Coverage

Collection tests pin:

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

Finalization tests pin:

- normal, empty-normal, and external physical slots;
- deterministic sorted names and preserved `came_from_commonjs` provenance;
- direct and multi-hop canonical equivalence retaining a name;
- any canonically distinct ESM alternative excluding a name;
- CJS conflicts being ignored by ESM ambiguity classification while every raw field stays unchanged; and
- every sorted key being present in the paired raw map.

The twenty-one-pass production trace pins `CollectResolvedExportsPass → BindImportsPass → FinalizeResolvedExportsPass → ComputeCjsRoutingPass → ResolveMemberExpressionsPass → CollectEntryExportRootsPass → CreateSyntheticExportStatementsPass → ReferenceNeededSymbolsPass` after normalization and side-effect analysis. Broader correctness and build gates remain in the pass-pipeline validation matrix; timing and memory are deferred until the final Link structure.

## Related

- [Pass-based pipeline implementation](../../pass-based-pipeline/implementation.md)
- [Pass-based pipeline design](../../pass-based-pipeline/design.md)
- [Module side effects](../module-side-effects/implementation.md)
- [Determine module formats](../determine-module-exports-kind/implementation.md)
