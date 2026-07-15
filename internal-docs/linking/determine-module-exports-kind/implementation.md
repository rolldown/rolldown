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
```

The driver currently projects the resulting compact formats, dynamic-export bits, and wrapper plan into the unchanged legacy module and metadata fields after the typed consumers finish. Wrapper declaration allocation and lazy normalization are still legacy steps in this transition. Once those writers migrate, only the post-lazy final formats and wrappers will be projected.

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

Lazy normalization remains the final format writer. In particular, object-form lazy JSON rebuilds its AST, local symbol database, and statement table, changes its final format to ESM, and invalidates any wrapper symbol or statement allocated before the rebuild. The migration therefore keeps `ModuleFormatsDraft → ModuleFormats` and `WrapperSeeds → WrapperPlan → WrapperDeclarationsDraft → ModuleWrappers` as explicit transitions instead of declaring the first classification final.

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
