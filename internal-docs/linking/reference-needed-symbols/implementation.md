# Reference needed symbols

## Purpose

`ReferenceNeededSymbolsPass` translates each module's linking decisions into per-statement dependencies. For every import record, given the importer/importee pair plus the final wrapper declaration returned after lazy normalization, it records:

- the `SymbolRef`s the lowered code will reference (`init_foo`, `require_foo`, namespace objects),
- the runtime helpers each statement depends on (`__toESM`, `__reExport`, `__toCommonJS`, `__name`, `__require`),
- whether the lowering forces the statement to be side-effecting,
- a stable `import_<name>` rename for external/CJS namespace bindings.

It owns and returns `SymbolRefDb` and `IndexStmtInfos`; it does not decide what is included. It separately mints sealed `StatementRuntimeRequirements` for `include_statements` and an ordered `ReferenceImportRecordPatches` artifact that delays only `CallRuntimeRequire` metadata mutation until the compatibility adapter.

Source: `crates/rolldown/src/stages/link_stage/passes/reference_needed_symbols.rs`.

## Pipeline placement

```text
… PlanModuleWrappingPass → CreateWrapperDeclarationsPass → NormalizeLazyExportsPass
  → DetermineModuleSideEffectsPass → representation compatibility projection
  → CollectResolvedExportsPass → BindImportsPass → FinalizeResolvedExportsPass
  → ComputeCjsRoutingPass → ResolveMemberExpressionsPass
  → CollectEntryExportRootsPass
  → CreateSyntheticExportStatementsPass
  → ReferenceNeededSymbolsPass   ← this pass
  → cross_module_optimization → include_statements → patch_module_dependencies
  → final compatibility projections and ReferenceImportRecordPatches::apply
```

This diagram records the current execution order, not a data dependency at every arrow. N does not read finalized `ResolvedExports`, CJS routing, member-expression resolutions, entry roots, shims, or external-star records. M finishes earlier because it must inspect the pre-synthetic statement graph for JSON object mutation and escape facts, while `CreateSyntheticExportStatementsPass` must hand the updated owned statement table to N.

Position is load-bearing in two directions:

1. **Final formats, wrappers, dynamic-export facts, side effects, CJS namespace merges, selected options, and the optional runtime `__require` reference must already exist as typed inputs.** Every CJS/ESM-wrap arm reads `ModuleWrappers`; wrapped ordinary imports read `ModuleSideEffects`; dynamic reexports read `DynamicExports`; merged CJS namespace imports read only `CjsNamespaceMerges::needs_interop`; and the remaining branches read only the exact output, tree-shaking, code-splitting, interop, keep-names, or require-polyfill scalars they implement. N never reads their legacy metadata projections.
2. **`include_statements` must run after.** Tree-shaking traverses the returned `stmt_info.referenced_symbols` and joins sealed `StatementRuntimeRequirements` against included statements. Without these outputs, wrappers and helpers would be silently dropped from the output.

## Pass contract

| Slot            | Type                              | Purpose                                                                                                                                                    |
| --------------- | --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `InputRead<'a>` | `ReferenceNeededSymbolsInput<'a>` | Borrows only typed module, representation, side-effect, and CJS-merge facts plus the optional runtime ref, output format, and two narrow option manifests. |
| `InputOwned`    | `ReferenceNeededSymbolsOwned`     | Moves in the unique `SymbolRefDb` and `IndexStmtInfos`; these are the only large mutable domains.                                                          |
| `OutputRead`    | `StatementRuntimeRequirements`    | Dense per-module statement/helper requirements sealed by the harness and borrowed only by inclusion.                                                       |
| `OutputOwned`   | `ReferenceNeededSymbolsOutput`    | Returns symbols and statements plus ordered `ReferenceImportRecordPatches`; the driver destructures this one-call envelope immediately.                    |
| `Error`         | `Infallible`                      | The Link boundary remains infallible; malformed internal layouts are hard invariants rather than recoverable partial results.                              |

Before mutation, N validates every dense length, normal/external slot shape, wrapper shape for externals, owner-local symbol slot, and the embedded `NormalModule::idx == physical ModuleIdx` identity. This preserves the legacy fail-closed behavior and prevents a malformed layout from being partially updated.

## Dispatch

For each `(importer, stmt_info, rec)` triple this pass dispatches on `rec.kind`, the importee's `WrapperDeclaration`, and (for `Import`) whether the record is a re-export-all (`export *`). The headings below use the equivalent `WrapKind` labels for readability.

### `Module::Normal` importees

- **`Import`, `WrapKind::None`, not reexport** — nothing recorded; flat ESM on both sides has no wrapper to call. Later stages still must check the canonical owner of live imported symbols: a non-wrapped barrel can forward a binding from a wrapped ESM module, and that owner still needs its `init_*()` call before the binding is read when the barrel does not execute in the same chunk — but only when the wrapper is reachable from the importer's chunk (declared there or registered as a cross-chunk import); otherwise the access flows through the barrel's namespace and the barrel's chunk performs the init itself.

  ```js
  // foo.js: export const x = 1;
  // index.js: import { x } from './foo';   → import { x } from './foo';   (unchanged here)
  ```

- **`Import`, `WrapKind::None`, reexport-all with `has_dynamic_exports`** — `side_effect=true`, set `ReExportDynamicExports`, push `__reExport`, importer & importee namespace refs. Covers the indirect-CJS case where a non-wrapped ESM intermediate forwards a wrapped CJS module's dynamic exports.

  ```js
  // bar.js (cjs): module.exports = { a: 1 };
  // foo.js: export * from './bar';          // wrap=cjs (forwarded)
  // index.js: export * from './foo';        // wrap=none, but bar's exports are dynamic
  //   → __reExport(index_exports, foo_exports);
  ```

- **`Import`, `WrapKind::Cjs`, not reexport** — push `wrapper_ref` (`require_foo`); push `__toESM` iff interop is needed; declare and rename the namespace ref to `import_<repr_name>`.

  ```js
  // foo.js (cjs): module.exports = { a: 1 };
  // index.js: import foo from './foo'; foo.a;
  //   → var import_foo = __toESM(require_foo()); import_foo.default.a;
  ```

- **`Import`, `WrapKind::Cjs`, reexport-all** — `side_effect=true`; push `wrapper_ref`; push `__toESM` and `__reExport`; when `treeshake.commonjs` is off, also push the importer namespace ref.

  ```js
  // foo.js (cjs): module.exports = { a: 1 };
  // index.js: export * from './foo';
  //   → __reExport(index_exports, __toESM(require_foo()));
  ```

- **`Import`, `WrapKind::Esm`, not reexport** — push `wrapper_ref` (`init_foo`); `side_effect = importee.side_effects.has_side_effects()`.

  ```js
  // foo.js (esm, wrapped): export const x = 1;
  // index.js: import { x } from './foo'; use(x);
  //   → init_foo(); use(x);
  ```

- **`Import`, `WrapKind::Esm`, reexport-all** — push `wrapper_ref` (`init_foo`); `side_effect=true` unconditionally (a reexport-all of a wrapped ESM importee always runs, regardless of `importee.side_effects`). Additionally, when the importee has dynamic exports, push `__reExport`, set `ReExportDynamicExports`, and push importer & importee namespace refs.

  ```js
  // foo.js (esm, wrapped, has dynamic exports via re-export of cjs)
  // index.js: export * from './foo';
  //   → init_foo(); __reExport(index_exports, foo_exports);
  ```

- **`Require`, `WrapKind::None`** — nothing; a `require` against a flat-ESM importee that wasn't promoted is a no-op at this layer.

- **`Require`, `WrapKind::Cjs`** — push `wrapper_ref` (`require_foo`).

  ```js
  // foo.js (cjs): module.exports = 1;
  // index.js: const f = require('./foo');
  //   → const f = require_foo();
  ```

- **`Require`, `WrapKind::Esm`** — push `wrapper_ref` and importee namespace ref; push `__toCommonJS` unless `IsRequireUnused`.

  ```js
  // foo.js (esm, wrapped): export const x = 1;
  // index.js: const f = require('./foo');
  //   → const f = (init_foo(), __toCommonJS(foo_exports));
  ```

- **`DynamicImport`, code-splitting on, CJS importee** — push `__toESM`; the chunk produced for the importee gets normalized at the call site.

  ```js
  // foo.js (cjs)
  // index.js: const f = await import('./foo');
  //   → const f = await import('./foo-chunk').then((m) => __toESM(m.default));
  ```

- **`DynamicImport`, code-splitting on, ESM/None importee** — nothing; the import becomes a chunk-level construct handled later.

- **`DynamicImport`, code-splitting off, CJS importee** — push `wrapper_ref` and `__toESM`.

  ```js
  // index.js: const f = await import('./foo');
  //   → const f = Promise.resolve().then(() => __toESM(require_foo()));
  ```

- **`DynamicImport`, code-splitting off, ESM importee** — push `wrapper_ref` and importee namespace ref.

  ```js
  // index.js: const f = await import('./foo');
  //   → const f = Promise.resolve().then(() => (init_foo(), foo_exports));
  ```

- **`AtImport` / `UrlImport`** — `unreachable!`. A JS module's import records cannot legally contain CSS-only kinds.

- **`NewUrl` / `HotAccept`** — no-op (asset reference / HMR metadata).

### `Module::External` importees

- **`Import`, reexport-all** — rename `rec.namespace_ref` to `import_<identifier_name>`. The `export *` itself is removed by a later pass; only the namespace name needs to be stable for de-conflicting.

  ```js
  // index.js: export * from 'lodash';
  //   → (removed; namespace ref renamed to `import_lodash`)
  ```

- **`Import`, named, output format ∈ `Cjs`/`Iife`/`Umd`** — `side_effect=true`; push `__toESM` iff `import_record_needs_interop` (default or namespace import).

  ```js
  // index.js: import lodash from 'lodash';                 // cjs output
  //   → const import_lodash = __toESM(require('lodash')); import_lodash.default;
  ```

- **`Require`, ESM-on-Node + `polyfill_require` option** — push `__require` symbol; defer `CallRuntimeRequire` meta on the import record so the finalizer rewrites the call.

  ```js
  // index.js: const fs = require('fs');                    // esm output, node platform
  //   → const fs = __require('fs');
  ```

- **`DynamicImport`, `Cjs` format + `!dynamic_import_in_cjs`** — push `__toESM`.

  ```js
  // index.js: const lodash = await import('lodash');       // cjs output, no dynamicImportInCjs
  //   → const lodash = await Promise.resolve().then(() => __toESM(require('lodash')));
  ```

- **Other external `rec.kind`** — no-op.

### Statement-level flags (checked independently of any specific record)

- `HasDummyRecord` → push `__require`. Set on `require(...)` calls without a resolvable target.
- `NonStaticDynamicImport` → push `__toESM`. For `import(foo)` / `import('a' + 'b')`.
- `keep_names && KeepNamesType` → push `__name`. The `keepNames` runtime implementation.

When `CjsNamespaceMerges::needs_interop(importee)` returns `Some`, that bit is authoritative for the `Import` / `WrapKind::Cjs` / non-reexport arm, overriding the per-record `import_record_needs_interop` check. The artifact records cross-importer agreement that several ESM importers can share one `__toESM` call; N cannot read or mutate its namespace-ref vectors.

## Invariants (the contract for `include_statements`)

After this pass:

1. **Wrapper and namespace `SymbolRef`s are in `referenced_symbols`.** If the lowered form mentions a wrapper call (`init_foo`, `require_foo`) or a namespace object (importer's or importee's `namespace_object_ref`), the corresponding `SymbolRef` is in `stmt_info.referenced_symbols`. Tree-shaking will drop anything not referenced; missing a push here = silently elided wrapper/namespace.
2. **Runtime helpers live in sealed `StatementRuntimeRequirements`, not `referenced_symbols`.** The lone exception is the external-runtime-`__require` polyfill arm, which pushes the resolved `__require` symbol onto `referenced_symbols` directly. `include_statements` joins the sealed per-module map against statement inclusion and pulls helpers in via `include_runtime_symbol`.
3. **The statement's evaluation flag is set whenever the lowered statement must run regardless of who reads it.** Includes `export * from 'cjs'`, `import 'esm-with-side-effects'`, all CJS-external imports under `Cjs/Iife/Umd`, and the dynamic-`__reExport` arms.
4. **Every CJS namespace import has a stable `import_<repr_name>` name.** Downstream rendering can rely on both `wrapper_ref` (set by wrapper declaration allocation) and the namespace name (set here) being settled.

A bug in any of (1)–(4) typically surfaces as a tree-shaking false-positive (helper or wrapper missing in output) or a de-conflict miss.

## Implementation constraints

- **Parallelism uses disjoint owned slots and no unsafe code.** The pass subtree has `#![forbid(unsafe_code)]`. N zips the shared physical `ModuleTable` with mutable owner-local symbol slots, mutable runtime-requirement slots, and mutable statement slots. The indexed zip preserves physical module identity; each closure mutates only the three owned slots at that position and reads all cross-module facts through shared narrow artifacts.
- **Physical and encounter order remain explicit.** The parallel result is one dense patch batch per physical module. Within a batch, statement order, each statement's import-record order, and repeated records are preserved exactly; no filtering, sorting, or deduplication changes the event transcript.
- **Cross-record metadata writes are deferred narrowly.** The runtime-`__require` polyfill arm emits only `{ importer, import_record }`. N neither owns nor mutates `ModuleTable`, and `ReferenceImportRecordPatches::apply` can set only `ImportRecordMeta::CallRuntimeRequire`. The driver applies those events after inclusion and dependency finalization at the compatibility boundary. A future mutation requires its own typed patch variant and lifecycle review rather than widening this event.
- **Symbol database reconstruction preserves global flags.** N temporarily owns the dense owner-local databases for the zipped walk, then rebuilds `SymbolRefDb` and restores `has_module_preserve_jsx` exactly.

## Coverage

Focused tests pin:

- exact physical batch, statement, record, duplicate-event, and late-patch order, including preservation of metadata written before adapter application;
- every normal-import wrapper branch, dynamic-export reexport branch, side-effect rule, CJS tree-shake gate, and merged-namespace interop override;
- `Require` and `DynamicImport` behavior across `None`, CJS, and ESM wrappers, with code splitting and output-format switches;
- external star renaming, CJS-family evaluation/interop, runtime-require polyfill, external dynamic-import conversion, and unresolved-record no-ops;
- statement-level dummy-require, non-static-dynamic-import, keep-names, and preserved JSX flags; and
- hard dense statement layout, external symbol-slot shape, and embedded normal-module index failures before mutation.

The exact twenty-one-pass trace pins M before synthetic statement creation and N immediately afterward. The pass-subtree test target also runs the production AST inventory, which keeps unsafe code, broad carriers, hidden pass declarations, and unapproved macros out of this implementation.

## Notes for editors

- **`CjsNamespaceMerges` overrides per-record interop.** When `needs_interop` returns `Some`, that bit is authoritative; a single per-record check would compute the wrong answer for the merged case.
- **`WrapKind::None` + `is_reexport_all` is intentional.** It exists for the "ESM importer re-exports a CJS-via-ESM intermediate that has dynamic exports" chain. Removing it breaks `__reExport` for indirect CJS reexports.
- **`commonjs_treeshake` gates the importer namespace-ref push in the `Cjs` reexport arm.** When on, `include_commonjs_export_symbol` handles that path; when off, the namespace ref is pushed unconditionally.
- **CSS import kinds are `unreachable!` here.** A JS module's `import_records` cannot legally contain `AtImport` / `UrlImport`; the panic is a guard against an upstream classification bug.

## Related

- [determine-module-exports-kind](../determine-module-exports-kind/implementation.md) — produces final typed formats and wrappers and the retained CJS namespace merge artifact.
- [module side effects](../module-side-effects/implementation.md) — documents the sealed final side-effect fact read directly by N.
- [module-execution-order](../module-execution-order/implementation.md) — orthogonal; `exec_order` is what `include_statements` uses to walk modules deterministically.
- `crates/rolldown/src/stages/link_stage/passes/plan_module_wrapping.rs` and `create_wrapper_declarations.rs` — plan wrapping and allocate paired wrapper symbol/statement identities.
- `crates/rolldown/src/stages/link_stage/passes/normalize_lazy_exports.rs` — preserves or invalidates wrapper identities atomically with lazy-export normalization, then returns final typed wrapper state.
- `crates/rolldown/src/stages/link_stage/tree_shaking/include_statements.rs` — the consumer of returned `referenced_symbols`, evaluation flags, and sealed `StatementRuntimeRequirements`.
