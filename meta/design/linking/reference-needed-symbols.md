# reference_needed_symbols

## Purpose

`reference_needed_symbols` translates each module's linking decisions into per-statement dependencies. For every import record, given the importer/importee pair plus the `WrapKind` chosen by `wrap_modules`, it records:

- the `SymbolRef`s the lowered code will reference (`init_foo`, `require_foo`, namespace objects),
- the runtime helpers each statement depends on (`__toESM`, `__reExport`, `__toCommonJS`, `__name`, `__require`),
- whether the lowering forces the statement to be side-effecting,
- a stable `import_<name>` rename for external/CJS namespace bindings.

It writes data; it does not decide what is included. `include_statements` is the next pass and consumes everything written here.

Source: `crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs`.

## Pipeline placement

```
… wrap_modules → generate_lazy_export → determine_side_effects
  → bind_imports_and_exports → create_exports_for_ecma_modules
  → reference_needed_symbols   ← this pass
  → cross_module_optimization → include_statements → patch_module_dependencies
```

Position is load-bearing in two directions:

1. **`wrap_kind` and `wrapper_ref` must already exist.** Every CJS/ESM-wrap arm reads `metas[importee.idx].wrap_kind()` and dereferences `wrapper_ref.unwrap()`. `wrap_modules` and `generate_lazy_export` populate them.
2. **`include_statements` must run after.** Tree-shaking traverses `stmt_info.referenced_symbols` and joins `depended_runtime_helper` against included statements. Without the data this pass writes, wrappers and helpers would be silently dropped from the output.

## Dispatch

For each `(importer, stmt_info, rec)` triple this pass dispatches on `rec.kind`, the importee's `WrapKind`, and (for `Import`) whether the record is a re-export-all (`export *`).

### `Module::Normal` importees

- **`Import`, `WrapKind::None`, not reexport** — nothing recorded; flat ESM on both sides has no wrapper to call. Later stages still must check the canonical owner of live imported symbols: a non-wrapped barrel can forward a binding from a wrapped ESM module, and that owner still needs its `init_*()` call before the binding is read when the barrel does not execute in the same chunk.

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

When `safely_merge_cjs_ns_map` has an entry for an importee, its `needs_interop` is authoritative for the `Import` / `WrapKind::Cjs` / non-reexport arm — overriding the per-record `import_record_needs_interop` check. The map records cross-importer agreement that several ESM importers can share one `__toESM` call.

## Invariants (the contract for `include_statements`)

After this pass:

1. **Wrapper and namespace `SymbolRef`s are in `referenced_symbols`.** If the lowered form mentions a wrapper call (`init_foo`, `require_foo`) or a namespace object (importer's or importee's `namespace_object_ref`), the corresponding `SymbolRef` is in `stmt_info.referenced_symbols`. Tree-shaking will drop anything not referenced; missing a push here = silently elided wrapper/namespace.
2. **Runtime helpers live in `depended_runtime_helper`, not `referenced_symbols`.** The lone exception is the external-runtime-`__require` polyfill arm, which pushes the resolved `__require` symbol onto `referenced_symbols` directly. `include_statements` joins this map against statement inclusion and pulls helpers in via `include_runtime_symbol`.
3. **`side_effect=true` is set whenever the lowered statement must run regardless of who reads it.** Includes `export * from 'cjs'`, `import 'esm-with-side-effects'`, all CJS-external imports under `Cjs/Iife/Umd`, and the dynamic-`__reExport` arms.
4. **Every CJS namespace import has a stable `import_<repr_name>` name.** Downstream rendering can rely on both `wrapper_ref` (set by `wrap_modules`) and the namespace name (set here) being settled.

A bug in any of (1)–(4) typically surfaces as a tree-shaking false-positive (helper or wrapper missing in output) or a de-conflict miss.

## Implementation constraints

- **Parallel mutation through raw-pointer casts.** The pass walks modules in parallel via `par_iter()` (which yields `&NormalModule`) but writes to two of the importer's fields, `stmt_infos` and `depended_runtime_helper`. Both are mutated through `addr_of!(...).cast_mut()`. Safety relies on per-module isolation: each closure mutates only the importer it was handed, and all cross-module reads (e.g. `self.module_table[importee_idx]`, `self.metas[..]`) touch other modules' state through `&self`. Don't widen the casts beyond those two fields without rebuilding the safety argument.
- **Cross-record metadata writes are deferred.** The closure cannot mutate `importer.import_records[rec_id].meta` directly (the iterator gives `&NormalModule`), so the runtime-`__require` polyfill arm collects `(rec_id, ImportRecordMeta::CallRuntimeRequire)` into a per-module `record_meta_pairs` and applies the writes serially after the parallel walk joins. This is the only deferred write; if a future arm needs to mutate other per-record state, route it through the same defer list rather than introducing a second mechanism.

## Notes for editors

- **`safely_merge_cjs_ns_map` overrides per-record interop.** When an entry exists for the importee, `info.needs_interop` is authoritative; a single per-record check would compute the wrong answer for the merged case.
- **`WrapKind::None` + `is_reexport_all` is intentional.** It exists for the "ESM importer re-exports a CJS-via-ESM intermediate that has dynamic exports" chain. Removing it breaks `__reExport` for indirect CJS reexports.
- **`commonjs_treeshake` gates the importer namespace-ref push in the `Cjs` reexport arm.** When on, `include_commonjs_export_symbol` handles that path; when off, the namespace ref is pushed unconditionally.
- **CSS import kinds are `unreachable!` here.** A JS module's `import_records` cannot legally contain `AtImport` / `UrlImport`; the panic is a guard against an upstream classification bug.

## Related

- [determine-module-exports-kind](./determine-module-exports-kind.md) — produces `wrap_kind` and `safely_merge_cjs_ns_map`.
- [module-execution-order](./module-execution-order.md) — orthogonal; `exec_order` is what `include_statements` uses to walk modules deterministically.
- `crates/rolldown/src/stages/link_stage/wrapping.rs` — populates `wrap_kind` and `wrapper_ref`.
- `crates/rolldown/src/stages/link_stage/tree_shaking/include_statements.rs` — the consumer of `referenced_symbols`, `side_effect`, and `depended_runtime_helper`.
