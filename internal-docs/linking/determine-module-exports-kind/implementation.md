# determine_module_exports_kind

## Summary

`determine_module_exports_kind` runs early in `LinkStage` and decides two things that gate everything downstream: each module's final `ExportsKind` (with one carefully-noted exception, see §"Invariants") and which modules need a `WrapKind::Esm` / `WrapKind::Cjs` wrapper at finalization. It is the place where the bundler stops _observing_ what the source said and starts _deciding_ how each module will be emitted. Most wrap decisions depend only on the _syntax_ of the `(importer, importee, ImportKind)` triple. The narrow JSON require-binding optimization also uses scanner metadata to keep an internal, non-entry JSON module in its split ESM shape when tree-shaking is enabled and every incoming record is a validated local property read.

Source: `crates/rolldown/src/stages/link_stage/determine_module_exports_kind.rs`.

Related code:

- `crates/rolldown/src/stages/link_stage/generate_lazy_export.rs` — the one stage allowed to revise `exports_kind` after this pass (see §"Invariants").
- `crates/rolldown/src/stages/link_stage/wrapping.rs` — consumes the `WrapKind` decisions made here.
- `LinkingMetadata::sync_wrap_kind` — the writer used for wrap state.

## Pipeline placement

The relevant prefix of `LinkStage::link()` (in `mod.rs`) runs roughly:

```
sort_modules
compute_tla
determine_module_exports_kind   <- this file
determine_safely_merge_cjs_ns
wrap_modules
generate_lazy_export
determine_side_effects
bind_imports_and_exports
create_exports_for_ecma_modules
reference_needed_symbols
include_statements
```

Position is load-bearing: `wrap_modules` propagates wrap requirements transitively through the graph using the `WrapKind`s set here as roots, and `bind_imports_and_exports` reads the `exports_kind` set here to decide how to thread re-exports through CJS namespace bindings.

## State this pass touches

`determine_module_exports_kind` writes:

- `module.exports_kind` for some normal modules through indexed mutable access.
- `self.metas[idx].wrap_kind` (and `original_wrap_kind`) via `LinkingMetadata::sync_wrap_kind`. **Not idempotent** — the last writer wins, so call order is part of the contract.

It does not touch symbol tables, tree-shaking flags, or chunk graph.

## Promotion + wrap rules

For each `(importer, importee, rec.kind)`:

| `rec.kind`                 | `importee.exports_kind` | Effect                                                                                                                                                                                                                      |
| -------------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Import`                   | `None` (non-lazy)       | Promote to `Esm`.                                                                                                                                                                                                           |
| `Import`                   | `Esm` / `CommonJs`      | No-op. (CJS-imported-by-ESM is wrapping work handled in `wrap_modules`.)                                                                                                                                                    |
| `Require`                  | `Esm`                   | Mark importee `WrapKind::Esm` (to satisfy `require()` of an ESM module).                                                                                                                                                    |
| `Require`                  | `CommonJs`              | Mark importee `WrapKind::Cjs`.                                                                                                                                                                                              |
| `Require`                  | `None`                  | Normally mark `WrapKind::Cjs` and promote to `CommonJs`. An internal, non-entry JSON module stays split ESM only when tree-shaking is enabled and every incoming record is a validated local require-binding property read. |
| `DynamicImport` (split)    | any                     | No-op. Code-splitting handles dynamic imports natively.                                                                                                                                                                     |
| `DynamicImport` (no split) | `Esm`                   | Mark `WrapKind::Esm`. `import()` lowers to `require + Promise.resolve(__toESM(...))`.                                                                                                                                       |
| `DynamicImport` (no split) | `CommonJs`              | Mark `WrapKind::Cjs`.                                                                                                                                                                                                       |
| `DynamicImport` (no split) | `None`                  | Mark `WrapKind::Cjs` and promote to `CommonJs`.                                                                                                                                                                             |
| `AtImport` / `UrlImport`   | —                       | `unreachable!` — see §"Why CSS import kinds are `unreachable!`".                                                                                                                                                            |
| `NewUrl` / `HotAccept`     | —                       | No-op (asset reference / HMR metadata, not a module-shape signal).                                                                                                                                                          |

After processing all import records, the importer is itself wrapped as CJS when:

- `importer.exports_kind == CommonJs`, **and**
- it is _not_ an entry, **or** the output format is `Esm`, **or** the output is `Iife`/`Umd` and the importer touches `module`/`exports`.

The "is entry + Esm output" branch is what allows `module.exports = ...` to keep working in a CJS-emit-as-ESM scenario; the `Iife`/`Umd` branch prevents leaking `module`/`exports` into the IIFE wrapper's outer scope.

> **Why "lazy export" is excluded from the `Import` + `None` arm:**
> Lazy-export modules are deferred ESM facades; promoting them here would short-circuit the dedicated lazy-export pass that runs later (`generate_lazy_export`), which performs additional restructuring that a naive `None → Esm` promotion would skip.

## Invariants (the contract for downstream stages)

After this pass completes:

1. **Non-lazy modules have their final `exports_kind`.** Every `Module::Normal` whose meta does **not** have `has_lazy_export()` has been classified — a residual `ExportsKind::None` means "no JS importer touched it; treat as a side-effect-only script."
2. **Lazy-export JSON may be provisionally promoted for require-binding DCE.** `generate_lazy_export` performs the JSON split and may revise its `wrap_kind` to `WrapKind::None`. Other lazy-export modules remain owned by that later pass.
3. **For every non-lazy `(importer, importee)` pair where wrapping is required, `metas[importee.idx].wrap_kind` is set.** `wrap_modules` may transitively propagate wrappers from there, but it will never _introduce_ a wrap that this pass missed.

Anything that breaks (1) or (3) is a bug _here_, not in the consumer.

## Why CSS import kinds are `unreachable!`

`Module::as_normal` filters out `Module::External`, `Module::CssModule`, and any non-JS module variant before this pass sees them. CSS dependencies are reached only via `ImportKind::AtImport` / `UrlImport`, which originate from CSS modules — not from JS. Therefore those kinds cannot appear in a JS module's `import_records`, and the panic is a guard against a misclassification upstream. `NewUrl` and `HotAccept` _do_ appear on JS modules but carry no exports/wrap implication, so they're explicit no-ops.

## Editing checklist

Things that are easy to break and worth re-checking when changing this file:

- **Order between `sync_wrap_kind` calls and `exports_kind` mutation.** Wrap decisions inside the `Require` / `DynamicImport` arms read `importee.exports_kind` _before_ any promotion would happen. Don't reorder.
- **The CJS-importer wrap rule** (after the per-record loop). The conjunction of conditions encodes three different output-format contracts; flattening it into a `match self.options.format` rewrite has tripped more than one reviewer. Add a regression test rather than refactoring blindly.
- **Don't promote lazy-export modules here except through the validated JSON require-binding path.** That path requires tree-shaking and excludes entries because their default object is externally mutable. Other `has_lazy_export()` modules remain owned by `generate_lazy_export`; promoting them prematurely will break the JSON-lazy and ESM-default code paths in that file.

## Related

- [module-execution-order](../module-execution-order/implementation.md)
