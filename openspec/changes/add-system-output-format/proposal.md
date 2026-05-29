## Why

Rolldown currently throws `Error: unimplemented: output.format: system` for any
build targeting SystemJS output, making it impossible to migrate Rollup-based
pipelines that rely on `format: 'system'` for browser delivery via
importmap-driven module resolution. SystemJS is the only format that supports
both code splitting and user-land importmap overrides without side-effects on
the host page — a pattern widely used in micro-frontend and plugin-based
architectures.

## What Changes

- Add `OutputFormat::System` as a new variant to the output format enum, making
  it a first-class peer of `esm`, `cjs`, `iife`, and `umd`.
- Implement `System.register(deps, factory)` chunk wrapper emission.
- Implement ordered `setters[]` generation — one setter per dependency, in
  dep-array order — capturing imported bindings into hoisted `var` declarations.
- Implement inline `exports("name", value)` live binding instrumentation: every
  assignment to an exported binding anywhere in the module body is wrapped so
  the SystemJS runtime receives live updates.
- Implement hoisted export block for function declarations (emitted before
  `execute` runs, leveraging JS function hoisting).
- Rewrite `import()` → `module.import()` and `import.meta` → `module.meta` for
  SystemJS module context.
- Implement `_starExcludes` null-prototype object for `export *` semantics.
- Add `output.name` support for named `System.register` calls.
- Add `systemNullSetters` option (already stubbed in binding layer as a
  comment).
- Wire into the existing Rollup compatibility test suite: un-skip all 16
  SystemJS fixtures currently listed in `ignored-by-unsupported-features.md`.
- Code splitting is supported natively (SystemJS is the only non-ESM format
  where it is valid); no forced `codeSplitting: false` for this format.

## Capabilities

### New Capabilities

- `system-format-wrapper`: The `System.register(name?, deps, factory)` chunk
  wrapper — parameters, strict mode injection, named vs anonymous registration,
  and the `execute` function shape including async/TLA support.
- `system-setters`: Ordered setter array generation — dep-array alignment, null
  setters (`systemNullSetters` option), re-export propagation inside setters,
  and `export *` via `_starExcludes`.
- `system-live-exports`: Per-assignment `exports()` injection in the module
  finalizer — all assignment forms (`=`, `+=`, `++`, `--`, destructuring, `for`
  loop vars), batch vs single export call forms, and hoisted
  function-declaration exports.
- `system-module-context`: `module.import()` for dynamic imports and
  `module.meta` / `module.meta.url` for import meta, including the `module`
  parameter deconfliction when user code has a local `module` variable.

### Modified Capabilities

## Impact

- **Rust core** (`crates/rolldown`): `OutputFormat` enum, `ecma_generator.rs`
  dispatch, new `format/system.rs`, module finalizer
  (`module_finalizers/mod.rs`) for live export injection,
  `render_chunk_exports.rs`, `determine_module_exports_kind.rs`,
  `reference_needed_symbols.rs`, `deconflict_chunk_symbols.rs`, `renamer.rs`,
  `prepare_build_context.rs`, `validate_options_for_multi_chunk_output.rs`.
- **Common types** (`crates/rolldown_common`): `OutputFormat` enum,
  `output_format.rs` helper methods (`source_type`,
  `should_call_runtime_require`, `keep_esm_import_export_syntax`, `is_esm`).
- **Binding layer** (`crates/rolldown_binding`): `binding_output_options/mod.rs`
  — un-comment `systemNullSetters`, add `system` to format string set.
- **TypeScript API** (`packages/rolldown`): `output-options.ts` `ModuleFormat`
  type, `normalized-output-options.ts` `InternalModuleFormat`,
  `bindingify-output-options.ts` `bindingifyFormat` switch.
- **Test suite** (`packages/rollup-tests`): Remove 16 entries from
  `ignored-by-unsupported-features.md`; add `'system'` to the `FORMATS` arrays
  in `test/form/index.js` and `test/chunking-form/index.js`.
- **No breaking changes** — all existing formats and options are unaffected.
