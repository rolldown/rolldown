# Import Attributes — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

Attributes are parsed per import record, travel through resolution as `ResolvedId.attributes`, and are folded into the `ModuleId` at a single point (`ResolvedId::finalize_attributes`) when `experimental.importAttributes` is on. From then on the composed id is the identity everywhere, while `ModuleId::raw_id()` gives back the file the module is read from.

## Representation

- `ImportAttributes` (`crates/rolldown_common/src/types/import_attributes.rs`) — `Option<Arc<[(CompactStr, CompactStr)]>>`, empty allocates nothing. Entries are kept sorted in the order JS enumerates the object Rollup stringifies (array-index keys numerically first, then the rest by UTF-16 code units), so equal attributes compare, hash and compose the same whatever order they were written in. The derived `Eq` / `Hash` rely on this. `get` is a linear scan, which is fine for the one or two entries an import has.
  - `compose_module_id` — exact `JSON.stringify` + `encodeURIComponent`.
  - `parse_module_id` — last `?attributes=` / `&attributes=`, accepted only if composing again yields the same id (Rollup's `getRawIdAndAttributes`).
  - `from_with_clause` — the attributes of a static clause, read the same way for the legacy `assert` keyword.
  - `to_with_clause` — `with { type: "json" }` for rendering externals.
  - `module_type` — the built-in module type of `attributes.type`. `load_source`, `builtin:asset-module` and `builtin:copy-module` all use it, so a `type` attribute beats `moduleTypes` everywhere.
- `ModuleId` (`crates/rolldown_common/src/types/module_id.rs`) — `{ inner, kind, raw_len }`. `raw_len` is the byte length of the raw id, or `NO_ATTRIBUTES`. Built only by `ModuleId::with_attributes`; `ModuleId::new` never parses, so a composed string passed to `new` is an opaque id. `as_path`, `representative_name`, `relative_path` and `StableModuleId` use `raw_id()`, so path logic ignores the suffix.
- TS helpers `stringifyModuleId` / `parseModuleId` (`packages/rolldown/src/utils/module-id.ts`) are copies of Rollup's and are exported from `rolldown/utils` only. The RFC places them in `@rolldown/pluginutils`, which lives outside this repo.

## Data flow

1. **Scan** (`crates/rolldown/src/ast_scanner/`) — static imports, `export * from`, `export {} from` set `ImportRecord.attributes` via `add_with_clause`. `import(x, { with: {...} })` reads a literal object with string values (`import_attributes_from_import_options`). A repeated key keeps its last value, as in any JS object. The legacy `assert` keyword is read like `with` and always warns `DEPRECATED_IMPORT_ASSERTION` (behind `checks.deprecatedImportAssertion`). With the option on, ignored `import()` options warn `INVALID_IMPORT_ATTRIBUTE` (behind `checks.invalidImportAttribute`), like Rollup does for the same cases. These are options that are not an object literal with a `with` object, and `with` entries without a string key and value. An `assert` options key is read like `with` and warns `DEPRECATED_IMPORT_ASSERTION`, also for an `import()` with a computed specifier, which has no import record.
2. **Resolve** (`crates/rolldown_plugin/src/utils/resolve_id_with_plugins.rs`) — `resolveId` gets `attributes`, `importerRawId`, `importerAttributes`. The plugin's `attributes` win over the requested ones. The built-in resolver resolves `importer_raw_id`, so relative imports from a module with attributes resolve next to the file.
3. **Finalize** (`resolve_id_check_external.rs`) — every return path calls `finalize_attributes(enabled)`. With the option on, or for an external, the id becomes `ModuleId::with_attributes(raw_id, attrs)`. Otherwise the attributes are dropped and the id stays raw. External checks run on the raw id. Entries (`utils/load_entry_module.rs`) and `this.load` go through the same call.
4. **Dedup** (`crates/rolldown/src/module_loader/resolve_utils.rs`) — dependencies are deduplicated by `(specifier, kind, attributes)`, so the same specifier with different attributes resolves twice.
5. **Load / transform** (`utils/load_source.rs`, `plugin_driver/build_hooks.rs`) — hooks get `id` (composed), `rawId` and `attributes`. The file is read from `raw_id`. `ImportAttributes::module_type` applies when the load hook gave no module type and before the extension guess. The oxc transformer gets the stable id without attributes, so `_jsxFileName` and similar names stay plain. `treeshake.moduleSideEffects` and `package.json#sideEffects` are matched against `raw_id`, as in Rollup.
6. **Externals** (`module_loader/external_module_task.rs`, `ecmascript/format/esm.rs`) — `ExternalModule.attributes` are the final attributes. The output path, `output.paths` and `output.globals` use `raw_id`. Every import / re-export of that external renders `importee.attributes.to_with_clause(..)` (the `import * as` that `ScopeHoistingFinalizer` emits for a namespace re-export gets the same clause, and its CJS `require()` uses `raw_id`), and `import()` of it gets `{ with: ... }` options from them (`ScopeHoistingFinalizer::rewrite_external_import_options`). Options the attributes were not read from are only replaced when there is something to render. With or without the option, `import()` of an external gets the path of a static import, and stays an `import()` while it has options, since `require()` cannot carry them. An `assert` clause or `import()` options key is rendered as `with`. `import()` with a computed specifier gets its `assert` key renamed to `with` too (`rename_import_options_assert_key`).

## Plugin context

- `this.resolve(source, importer, { attributes })` — `importer` may be a composed id (parsed back for `importerRawId` / `importerAttributes`) or `{ rawId, attributes }`; the result is `{ id: rawId, attributes, ... }`.
- `this.load({ id, attributes })` — `id` may be composed or `{ rawId, attributes }`. The `attributes` option is used only when `id` has none, so the result of `this.resolve` can be passed as is. The Rust side (`NativePluginContext::load`) finalizes a `ResolvedId` the same way resolution does and returns the composed id it waited on.
- `resolveDynamicImport` gets the same `attributes`, `importerRawId` and `importerAttributes` as `resolveId`, and the `attributes` it returns apply the same way.
- JS-side module options (`meta`, `moduleSideEffects`, …) are keyed by `PluginContextData.toModuleId(rawId, attributes)`, which composes only when the option is on, matching the Rust id.
- A plugin that stores state under the id it returns from `resolveId` must look it up by `rawId` in `load` / `transform`: `id` has the attributes appended. `rolldown_plugin_data_url` does this; checks that only look at the extension or a `?query` (e.g. `vite_json`, `vite_transform` via `clean_url`) keep working.

## Binding

`BindingImportAttributes` is `IndexMap<String, String>` (a JS `Record<string, string>`), converted at the boundary in `crates/rolldown_binding/src/options/plugin/types/binding_import_attributes.rs`. Hooks run for every module, so a module without attributes must not pay for them. The binding leaves out empty attributes and a `rawId` equal to the id, and `bindingify-build-hooks.ts` fills in `{}` / the id. The load hook gets a third `BindingHookLoadExtraArgs { rawId, attributes }` argument only for a module with attributes. Builtin plugins called from JS (`binding_callable_builtin_plugin.rs`) split a composed id with `parse_module_id`. Their `resolveId` still gets `{}` as `attributes`, because `BindingHookJsResolveIdOptions` has no field for them yet.

## Invariants

- With the option off, only externals keep `ResolvedId.attributes` after finalization and have `raw_len != NO_ATTRIBUTES`.
- `ModuleId` equality and hashing are on the composed string; `raw_len` is derived data and never part of identity.
- Composition happens only in `ResolvedId::finalize_attributes`; anything producing a module id from a resolution must go through it.

## Tests

- Rust: `crates/rolldown/tests/rolldown/topics/import_attributes/` (`module_identity`, `builtin_type`, `builtin_type_vs_module_types`, `external`, `external_globals`, `external_reexport`, `dynamic_import`, `data_url`, `invalid_import_attribute`), `function/external/assert_clause_is_treated_as_with`, `function/external/import_attributes_kept_without_option`, plus unit tests in `import_attributes.rs`.
- Node: `packages/rolldown/tests/fixtures/topics/import-attributes/` (hook args, `resolveId` override for static and dynamic imports, `resolveDynamicImport`, `output.paths` of externals, `this.resolve` / `this.load`, option disabled) and `packages/rolldown/tests/utils/module-id.test.ts`.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
- [module-id](../module-id/implementation.md) — `ModuleId` kinds and path identity
