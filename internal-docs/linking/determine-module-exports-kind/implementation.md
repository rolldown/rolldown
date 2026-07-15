# Determine module formats

## Summary

`DetermineModuleFormatsPass` classifies every normal module's link-time `ExportsKind` and produces the initial wrapper requirements that follow directly from import syntax and the output format. It does not mutate `ModuleTable` or linking metadata. Instead, it returns two owned, non-clone artifacts: `ModuleFormatsDraft` and `WrapperSeeds`.

The output is deliberately still a draft. Lazy-export normalization is the final format writer, while wrapper planning, declaration allocation, and lazy JSON rebuilding still have to run before wrapper state is final.

Source: `crates/rolldown/src/stages/link_stage/passes/determine_module_formats.rs`.

## Pipeline placement

The relevant typed prefix of `LinkStage::link()` is:

```text
CanonicalizeEntriesPass → EntryPlanDraft
ComputeModuleExecutionOrderPass borrows EntryPlanDraft
ComputeTlaPass
DetermineModuleFormatsPass borrows EntryPlanDraft
  ├─ ComputeCjsNamespaceMergesPass borrows ModuleFormatsDraft
  ├─ ComputeDynamicExportsPass borrows ModuleFormatsDraft
  └─ PlanModuleWrappingPass borrows ModuleFormatsDraft and consumes WrapperSeeds
       └─ CreateWrapperDeclarationsPass consumes WrapperPlan, SymbolRefDb, and IndexStmtInfos
            └─ NormalizeLazyExportsPass borrows EntryPlanDraft, CjsNamespaceMerges, and GlobalConstantsDraft; consumes format and wrapper drafts with the module, AST, symbol, and statement tables
                 ├─ DetermineModuleSideEffectsPass borrows final ModuleWrappers and sealed DynamicExports
                 └─ CollectResolvedExportsPass borrows the final ModuleTable
DetermineModuleSideEffectsPass and representation projection ··· current driver order only ···> CollectResolvedExportsPass
```

`NormalizeLazyExportsPass` performs the final draft-to-final transition. `DetermineModuleSideEffectsPass` is the last typed reader of final wrappers and sealed dynamic exports. `CollectResolvedExportsPass` then reads the identity-stable final module table and returns owned maps. The driver projects side effects, post-lazy formats, dynamic-export bits, wrapper declarations, and resolved exports into unchanged legacy module and metadata fields before converting the still-typed CJS namespace, entry, and constant artifacts for their remaining legacy consumers.

## Pass contract

| Slot            | Type                                 | Purpose                                                                                                                 |
| --------------- | ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `InputRead<'a>` | `DetermineModuleFormatsInput<'a>`    | Borrows `ModuleTable` and the unique `EntryPlanDraft`; copies only `OutputFormat` and the code-splitting-disabled flag. |
| `InputOwned`    | `()`                                 | The pass owns no entity table.                                                                                          |
| `OutputRead`    | `()`                                 | Format and wrapper values are still mutable drafts.                                                                     |
| `OutputOwned`   | `(ModuleFormatsDraft, WrapperSeeds)` | Two separate compact domains with one writer and explicit later consumers.                                              |
| `Error`         | `Infallible`                         | The link boundary remains infallible.                                                                                   |

`ModuleFormatsDraft` is a dense `IndexVec<ModuleIdx, Option<ExportsKind>>`. `Some(ExportsKind::None)` means a normal module whose format is still `None`; `None` means the slot belongs to an external module. Keeping those states distinct prevents external modules from being classified as ordinary `ExportsKind::None` modules.

`WrapperSeeds` owns a dense `IndexVec<ModuleIdx, WrapperStateDraftSlot>`. Each slot carries `kind: Option<WrapKind>` and `required_by_other_module: bool`: a normal module starts with `Some(WrapKind::None)`, while an external module uses `None`; the independent required flag can still become true for an external target reached by `require`. `DetermineModuleFormatsPass` produces the initial kind, and `PlanModuleWrappingPass` uniquely consumes the artifact, mutates both fields, and reuses the same `IndexVec` allocation as `WrapperPlan` instead of allocating or copying a second dense table.

Neither artifact exposes a constructor, `Clone`, `Default`, or mutable access.

## Promotion and seed rules

After unresolved and external targets are skipped, each import record applies the following rule to the current draft value:

| Import kind                              | Current target format | Result                                                                       |
| ---------------------------------------- | --------------------- | ---------------------------------------------------------------------------- |
| `Import`                                 | `None`, non-lazy      | Promote the format to `Esm`; leave the wrapper seed unchanged.               |
| `Import`                                 | `None`, lazy          | Leave both values unchanged for lazy normalization.                          |
| `Import`                                 | `Esm` or `CommonJs`   | Leave both values unchanged.                                                 |
| `Require`                                | `Esm`                 | Seed `WrapKind::Esm`.                                                        |
| `Require`                                | `CommonJs`            | Seed `WrapKind::Cjs`.                                                        |
| `Require`                                | `None`                | Promote to `CommonJs` and seed `WrapKind::Cjs`.                              |
| `DynamicImport`, code splitting enabled  | Any                   | Leave both values unchanged.                                                 |
| `DynamicImport`, code splitting disabled | Any                   | Apply the same rule as `Require`.                                            |
| `NewUrl` or `HotAccept`                  | Any                   | Leave both values unchanged.                                                 |
| `AtImport` or `UrlImport`                | Any                   | Preserve the legacy unreachable invariant for malformed normal-module input. |

After all records for one importer have been processed, a CommonJS importer is itself seeded as `WrapKind::Cjs` when it is not an entry, when the output is ESM, or when the output is IIFE/UMD and the module uses either `module` or `exports`.

Entry membership comes from `EntryPlanDraft::contains_root`, so user-defined, dynamic-import, and emitted entries all receive the same exemption. It is not derived from `user_defined_entry_modules`.

## Order is semantic

The pass is intentionally serial. It walks the physical `ModuleTable` order, then each module's original import-record order, and every record reads the draft after all earlier promotions. A promotion made by an earlier importer must be visible to later importers.

For a target whose initial format is `None`:

- `Import` followed by `Require` produces `Esm` plus `WrapKind::Esm`.
- `Require` followed by `Import` produces `CommonJs` plus `WrapKind::Cjs`.

This first-observer behavior is why the pass cannot classify modules independently, collect promotions in parallel, or apply a later reduction without changing semantics. Both import-record order and module order have focused tests.

## CSS import-kind invariant

`AtImport` and `UrlImport` are constructively absent from valid normal-module link input. CSS module types are rejected with a structured diagnostic before an ECMA view is created, and every normal-module raw record comes from the ECMA scanner, whose record-producing paths emit only `Import`, `DynamicImport`, `Require`, `NewUrl`, and `HotAccept`. The loader preserves each kind and also has an earlier redundant guard against CSS kinds.

The old link method nevertheless treated synthetic malformed `ModuleTable` values as unreachable at this exact classification point. The pass preserves those two existing branches so the extraction does not weaken or delay the invariant. The production-source inventory admits only the fully qualified `std::unreachable!` path, only in `determine_module_formats.rs`, and only with the two legacy messages; all other production expression macros remain rejected unless separately listed in the closed allowlist.

## Downstream lifecycle

The current typed consumers are:

- `ComputeCjsNamespaceMergesPass`, which reads `CommonJs` formats and returns an owned map that must eventually move into `LinkStageOutput`.
- `ComputeDynamicExportsPass`, which preserves the existing export-star DFS and returns a sealed dense bitset.
- `PlanModuleWrappingPass`, which consumes `WrapperSeeds`, propagates wrapper requirements and `required_by_other_module`, and returns `WrapperPlan`.
- `CreateWrapperDeclarationsPass`, which consumes that compact plan plus the symbol and statement tables, preserves module-order allocation, and returns the same tables with a dense `WrapperDeclarationsDraft`. Each declaration is `None`, `Cjs { wrapper_ref, wrapper_stmt_info }`, or `Esm { wrapper_ref, wrapper_stmt_info }`, so kind and both identities cannot diverge; the required flag remains independent for external modules.
- `NormalizeLazyExportsPass`, which borrows the entry, CJS-merge, and global-constant identity carriers; atomically owns the module, AST, statement, symbol, format-draft, and wrapper-draft domains; and returns the same large allocations with final `ModuleFormats`, `ModuleWrappers`, binding-only `NonSplittableJsonDefaults`, and finalizer-only `LazyJsonExportInitializers`.
- `DetermineModuleSideEffectsPass`, which borrows final `ModuleWrappers` and sealed `DynamicExports`, preserves the legacy ordered recursion and cache, and returns sealed dense `ModuleSideEffects` for one compatibility projection.
- `CollectResolvedExportsPass`, which borrows the final `ModuleTable`, preserves path-local export-star traversal, and returns owned dense `ResolvedExportsDraft` for a checked no-clone compatibility projection.

Lazy normalization is the implemented final format writer. A non-CJS object-form lazy JSON module is rebuilt into independently tree-shakeable bindings only when its recursive JSON AST, owner-local side tables, statement and reverse-index shape, facade-symbol database, and optional ESM wrapper exactly match the pristine loader state and no borrowed identity carrier names that module as an owner. That path replaces the local semantic database and whole statement table, changes the format to ESM, recreates namespace/default/HMR facades, and clears the invalidated wrapper declaration. Every transformed or otherwise non-pristine module instead keeps its existing identities. Before `transformAst`, Parse records the loader-created payload statement's arena address in a side channel without changing the AST exposed to hooks. The address survives statement moves and in-place edits. If a hook replaces the whole statement and loses that identity, Parse accepts the result only when exactly one expression-statement candidate remains; otherwise it returns `TRANSFORM_ERROR` before Scan instead of guessing. Scan then maps the resolved body index to exactly one `StmtInfoMeta::LazyExportPayload` or fails the build. Lazy normalization requires that unique marker and wraps the exact expression; Link has no expression-order fallback. JSON property exports use ordinary appended facade bindings and synthetic initializer statements, preserve plugin-defined exports, support arbitrary string names, and snapshot from the materialized default object immediately after its payload. Accessor-shaped objects also produce a sparse binding-only set that prevents default-property reads from being rewritten to snapshots. Generate consumes the separate sparse initializer recipe after inclusion, emits only retained bindings, and drops it immediately after parallel module finalization. `__proto__` is computed so it remains an own data property, while CJS/IIFE/UMD export-name adapters escape every `Object.defineProperty` key. The pass therefore completes `ModuleFormatsDraft → ModuleFormats` and `WrapperDeclarationsDraft → ModuleWrappers` only after the final possible identity change.

## Editing checklist

- Preserve physical module order and import-record order.
- Read the evolving draft, not the scan-time format, for every record.
- Run the CommonJS importer-entry rule only after all records for that importer.
- Keep external slots distinct from normal `ExportsKind::None` slots.
- Keep static imports of lazy `None` modules unpromoted.
- Apply disabled dynamic imports exactly like `Require`.
- Keep entry membership tied to the canonical entry draft.
- Do not add a broad options object, linking metadata, `LinkStage`, a panic path, or a clone of either output artifact.
- Do not parallelize this pass. Any future concurrency belongs between independent passes after this serial classification is complete and must be justified by measurement.

## Related

- [Pass-based pipeline implementation](../../pass-based-pipeline/implementation.md)
- [Module execution order](../module-execution-order/implementation.md)
- [Resolved exports](../resolved-exports/implementation.md)
