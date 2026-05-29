## Commit discipline

Each group should be committed as a unit once all its tasks are green —
including fixtures passing. Suggested commit message format:

```
feat(system-format): <group title>

e.g.
feat(system-format): foundation — enum, types, and skeleton that compiles
feat(system-format): wrapper structure and configuration
feat(system-format): link stage and deconfliction
feat(system-format): setters and dependency array
feat(system-format): module.import() and module.meta rewriting
feat(system-format): live export instrumentation
feat(system-format): code splitting
feat(system-format): edge cases and remaining rollup fixtures
feat(system-format): top-level await
feat(system-format): final sweep and cleanup
```

This keeps bisect meaningful: every commit in the log is a self-contained,
test-verified increment.

---

## How to run a single fixture during development

```bash
just t-run crates/rolldown/tests/rolldown/function/format/system/<fixture-name>/_config.json
```

On first run with no `artifacts.snap`, the snapshot is created — inspect it. If
the output is wrong, delete `artifacts.snap` and fix the code. Re-run until the
snapshot matches the Rollup reference output.

For rollup compat tests (once `system` is added to `FORMATS`):

```bash
pnpm --filter rollup-tests test -- --grep "system-export-rendering"
```

---

## 1. Foundation: Enum, Types, and a Skeleton That Compiles

The goal of this group is to make `format: "system"` accepted without a runtime
error, producing empty (but structurally valid) output. Every subsequent group
adds correctness on top of this skeleton.

- [x] 1.1 Add `System` variant to `OutputFormat` enum in
      `crates/rolldown_common/src/inner_bundler_options/types/output_format.rs`
- [x] 1.2 Implement helper methods on `OutputFormat::System`: `as_str()` →
      `"system"`, `source_type()`, `should_call_runtime_require()` → `false`,
      `keep_esm_import_export_syntax()` → `false`, `is_esm()` → `false`
- [x] 1.3 Add `"system"` mapping in the Rust binding layer
      (`crates/rolldown_binding/src/utils/normalize_binding_options.rs`) and
      update the error message to include `"system"` in the valid values list
- [x] 1.4 Add `'system'` to `ModuleFormat` union type in
      `packages/rolldown/src/options/output-options.ts`, to
      `InternalModuleFormat` in `normalized-output-options.ts`, and add
      `case 'system': return 'system'` to `bindingifyFormat()` in
      `bindingify-output-options.ts`
- [x] 1.5 Create `crates/rolldown/src/ecmascript/format/system.rs` with a stub
      `render_system()` function that returns an empty `SourceJoiner`; add
      `pub mod system;` to `format/mod.rs`
- [x] 1.6 Add `OutputFormat::System` arm to the dispatch in
      `crates/rolldown/src/ecmascript/ecma_generator.rs` calling
      `render_system()`
- [x] 1.7 Add exhaustive `OutputFormat::System` arms to every existing `match`
      that does NOT have a wildcard (compiler will guide you — fix all
      `non-exhaustive patterns` errors); use `todo!()` stubs where behaviour is
      unclear — replace these in later groups
- [x] 1.8 Add `OutputFormat::System` to the `render_chunk_exports` and
      `render_wrapped_entry_chunk` match arms in
      `crates/rolldown/src/utils/chunk/render_chunk_exports.rs` returning `None`
      (export postamble is handled inline for SystemJS, not here)
- [x] 1.9 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/empty_module/_config.json`
      with `{ "config": { "format": "system" } }` and a `main.js` with no
      imports/exports. Run `just t-run` — it should no longer throw
      `Error: unimplemented`. Snapshot will be nearly empty; that is expected.

## 2. Wrapper Structure and Configuration

Implement the `System.register(deps, factory)` skeleton with correct factory
parameters, strict mode, and the return object shape — no setters or live
exports yet.

- [x] 2.1 Implement anonymous `System.register([], (function() {` wrapper open
      and `}));` close in `render_system()`
- [x] 2.2 Implement named registration: when `output.name` is set, emit
      `System.register('name', [], (function() {` — name as plain string
      literal, no namespace decomposition (unlike IIFE/UMD)
- [x] 2.3 Implement factory parameter logic: compute `has_exports` and
      `uses_module_context` (dynamic import or import.meta present); emit
      `(exports)`, `(exports, module)`, `(module)`, or `()` accordingly
- [x] 2.4 Implement `'use strict';` injection inside factory, controlled by
      `output.strict` (default `true`)
- [x] 2.5 Emit `return { setters: [], execute: (function () {` and closing
      `}) };` as placeholders (setters populated in group 4)
- [x] 2.6 Apply banner/footer/intro/outro hooks in correct order: banner before
      `System.register`, intro inside factory before module sources, outro after
      module sources, footer after closing `});`
- [x] 2.7 Do NOT add `OutputFormat::System` to the forced-`codeSplitting: false`
      guard in `crates/rolldown/src/utils/prepare_build_context.rs`
- [x] 2.8 Do NOT add `OutputFormat::System` to the multi-chunk validation block
      in
      `crates/rolldown/src/utils/chunk/validate_options_for_multi_chunk_output.rs`
- [x] 2.9 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/basic_wrapper/`
      with `main.js` containing `export const x = 1;` and `_config.json`
      `{ "config": { "format": "system" } }`. Run `just t-run`. Verify snapshot
      shows correct `System.register([],` outer shape and
      `(function (exports) {` factory parameter.
- [x] 2.10 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/named_register/`
      with `_config.json`
      `{ "config": { "format": "system", "name": "my-lib" } }`. Verify snapshot
      shows `System.register('my-lib',`.
- [x] 2.11 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/no_strict/` with
      `_config.json` `{ "config": { "format": "system", "strict": false } }`.
      Verify snapshot does NOT contain `'use strict'`.

## 3. Link Stage and Deconfliction

Ensure SystemJS is treated correctly during the link and symbol-resolution
stages.

- [x] 3.1 Add `OutputFormat::System` arm to
      `crates/rolldown/src/stages/link_stage/determine_module_exports_kind.rs` —
      System behaves like ESM (no forced CJS wrapping of imported modules)
- [x] 3.2 Add `OutputFormat::System` handling in
      `crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs` — do
      NOT inject `__toESM`, `__require`, or `__commonJS` helpers
- [x] 3.3 Add `OutputFormat::System` arm to
      `crates/rolldown/src/stages/link_stage/create_exports_for_ecma_modules.rs`
      — System needs namespace symbols for inter-chunk exports similarly to ESM
- [x] 3.4 Add `"module"` and `"exports"` to the SystemJS reserved name set in
      `crates/rolldown/src/utils/renamer.rs` and
      `crates/rolldown/src/utils/chunk/deconflict_chunk_symbols.rs`
- [x] 3.5 Un-comment and implement `systemNullSetters: bool` in
      `crates/rolldown_binding/src/options/binding_output_options/mod.rs`;
      thread through to `NormalizedBundlerOptions` with default `true`; expose
      in TypeScript `OutputOptions` and `bindingify-output-options.ts`
- [x] 3.6 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/deconflict_module_name/`
      with `main.js` containing `const module = 1; export { module as val }` and
      a dynamic import. Run `just t-run`. Verify snapshot renames the user's
      `module` variable to `module$1` and the factory parameter remains
      `module`.

## 4. Setters and Dependency Array

Implement ordered setter generation. Each fixture in this group is written
first, then the setter code is implemented until the snapshot matches the Rollup
reference.

- [ ] 4.1 Implement ordered deps array construction in `render_system()` from
      chunk external imports and internal chunk dependencies, in consistent
      iteration order
- [ ] 4.2 Emit hoisted `var` declarations for all import bindings (before
      `return`)
- [ ] 4.3 Implement setter generation — one entry per dep, same order as deps
      array: named/default/namespace bindings → `module.prop` assignment;
      side-effect-only → `null` (systemNullSetters=true) or `function(){}`
      (false)
- [ ] 4.4 Implement re-export propagation in setters: single re-export →
      `exports('name', module.prop)`; multiple re-exports from same dep → batch
      object form `exports({ a: module.a, b: module.b })`
- [ ] 4.5 Implement `_starExcludes` null-prototype object for `export *`:
      collect all own export names plus `"default"`, emit
      `var _starExcludes = { __proto__: null, default: 1, ownExport: 1 };`;
      setter loops over `module` keys filtering through `_starExcludes`
- [ ] 4.6 Add debug assertion (dev builds only):
      `assert_eq!(deps.len(), setters.len())`
- [ ] 4.7 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/named_import/` —
      imports `{ foo }` from an external, re-exports it. Verify deps array, var
      declaration, and setter assignment are all present and correctly ordered.
- [ ] 4.8 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/side_effect_import/`
      — `import './side-effect'` with no consumed bindings. Verify setter is
      `null`.
- [ ] 4.9 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/null_setters_false/`
      with `_config.json`
      `{ "config": { "format": "system", "systemNullSetters": false } }`. Verify
      setter is `function () {}` not `null`.
- [ ] 4.10 **Rollup fixture (red → green)**: Enable `system` in
      `packages/rollup-tests/test/form/index.js` `FORMATS` array (add
      `'system'`). Remove `rollup@form@system-null-setters` from
      `ignored-by-unsupported-features.md`. Run
      `pnpm --filter rollup-tests test -- --grep "system-null-setters"` and
      confirm it passes.
- [ ] 4.11 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-reexports` from `ignored-by-unsupported-features.md`.
      Run that fixture and confirm it passes (validates re-export setter batch
      form and `_starExcludes`).

## 5. module.import() and module.meta Rewriting

Implement dynamic import and import.meta rewriting, and the `module` parameter
presence tracking.

- [ ] 5.1 Add logic to track whether a chunk uses dynamic import or
      `import.meta` (store as a flag on `GenerateContext` or compute from the
      module table); use this flag in group 2 task 2.3 for factory parameter
      emission
- [ ] 5.2 Add `OutputFormat::System` branch in the module finalizer
      (`crates/rolldown/src/module_finalizers/mod.rs`) to rewrite `import()`
      expressions to `module.import()`
- [ ] 5.3 Add `OutputFormat::System` branches for `import.meta` → `module.meta`
      and `import.meta.url` → `module.meta.url`
- [ ] 5.4 Add `OutputFormat::System` branch for
      `import.meta.ROLLUP_FILE_URL_<refId>` →
      `new URL('<path>', module.meta.url).href`
- [ ] 5.5 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/dynamic_import/`
      with `main.js` that does `const m = await import('./lazy.js')` and a
      `lazy.js`. Verify snapshot shows `module.import('./lazy.js')` and factory
      signature includes `module` parameter.
- [ ] 5.6 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/import_meta_url/`
      with `main.js` that uses `import.meta.url`. Verify snapshot shows
      `module.meta.url` and factory includes `module` parameter.
- [ ] 5.7 **Rollup fixture (red → green)**: Remove
      `rollup@form@import-namespace-systemjs` from
      `ignored-by-unsupported-features.md`. Run and confirm it passes.

## 6. Live Export Instrumentation

This is the hardest group. Write each fixture first, run it to see wrong output,
then implement the specific transformation. Use the Rollup reference fixtures as
the correctness oracle wherever they exist.

- [ ] 6.1 **Inspect Rollup fixture**: Before writing any code, inspect the
      `system-export-rendering` Rollup fixture output (in the initialized
      submodule at
      `rollup/test/form/samples/system-export-rendering/_expected/system.js`) to
      pin down the exact form for postfix `x++`/`x--` and destructuring exports.
      Record findings as comments in `format/system.rs`.
- [ ] 6.2 Add a pre-pass or extend link-stage metadata to identify, per module,
      the set of exported mutable bindings requiring live `exports()` wrapping —
      reuse `must_keep_live_binding` from `render_chunk_exports.rs`; `const` and
      provably-non-reassigned bindings are excluded
- [ ] 6.3 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_let_init/` —
      `export let x = 10`. Expected: `let x = exports('x', 10)`. Implement
      `exports()` wrapping for variable initializers in the finalizer.
- [ ] 6.4 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_reassign/` —
      `export let x = 1; x = 2`. Expected: `exports('x', x = 2)`. Implement
      wrapping for simple assignment expressions.
- [ ] 6.5 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_compound_assign/`
      — `export let n = 0; n += 1`. Expected: `exports('n', n += 1)`. Implement
      wrapping for compound assignment operators.
- [ ] 6.6 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_prefix_inc/`
      — `export let n = 0; ++n`. Expected: `exports('n', ++n)`. Implement prefix
      increment/decrement wrapping.
- [ ] 6.7 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_postfix_inc/`
      — `export let n = 0; n++`. Implement postfix wrapping using the exact form
      determined in task 6.1.
- [ ] 6.8 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_destructuring/`
      — `export let a, b; [a, b] = fn()`. Implement destructuring assignment
      export wrapping.
- [ ] 6.9 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_function_hoisted/`
      — `export function greet() {}`. Expected: hoisted
      `exports('greet', greet)` block before execute body; function declaration
      unchanged inside execute. Implement hoisted function-declaration export
      block.
- [ ] 6.10 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_class/` —
      `export class Foo {}`. Expected: `class Foo {} exports('Foo', Foo);`
      inside execute. Implement class-declaration export.
- [ ] 6.11 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_const/` —
      `export const PI = 3.14`. Verify no per-assignment wrapping is emitted;
      only a single `exports('PI', 3.14)` at initializer.
- [ ] 6.12 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_default_expr/`
      — `export default 42`. Expected: `exports('default', 42)` inline.
- [ ] 6.13 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/export_uninitialized/`
      — `export let x`. Verify correct handling of uninitialized exports.
- [ ] 6.14 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-export-rendering` from
      `ignored-by-unsupported-features.md`. Run it and confirm it passes — this
      is the comprehensive live export correctness test.
- [ ] 6.15 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-export-declarations` from
      `ignored-by-unsupported-features.md`. Run and confirm it passes.
- [ ] 6.16 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-export-destructuring-declaration` from
      `ignored-by-unsupported-features.md`. Run and confirm it passes.
- [ ] 6.17 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-multiple-export-bindings` from
      `ignored-by-unsupported-features.md`. Run and confirm it passes.
- [ ] 6.18 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-uninitialized` from
      `ignored-by-unsupported-features.md`. Run and confirm it passes.

## 7. Code Splitting

Verify that SystemJS correctly handles multiple chunks (static deps and dynamic
imports as separate split points).

- [ ] 7.1 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/code_split_dynamic/`
      with `main.js` dynamically importing `./lazy.js` which exports a value.
      Verify: two chunks produced; main uses `module.import('./lazy.js')`; lazy
      chunk wraps its export in `System.register`.
- [ ] 7.2 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/code_split_static/`
      where a shared chunk is statically imported by two entry points. Verify
      the shared chunk appears in each entry's deps array with a corresponding
      setter.
- [ ] 7.3 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/circular_chunks/`
      with two chunks that circularly import from each other. Verify both chunks
      list each other in their deps arrays and setters capture the live
      bindings.
- [ ] 7.4 **Rollup fixture (red → green)**: Add `'system'` to the formats list
      in `packages/rollup-tests/test/chunking-form/index.js`. Run chunking-form
      tests for system format and confirm they pass.

## 8. Edge Cases and Remaining Rollup Fixtures

Handle the remaining rollup compatibility fixtures and edge cases not covered
above.

- [ ] 8.1 **Rollup fixture (red → green)**: Remove `rollup@form@system-comments`
      from `ignored-by-unsupported-features.md`. Run and confirm leading comment
      placement is correct.
- [ ] 8.2 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-default-comments` from
      `ignored-by-unsupported-features.md`. Run and confirm default export
      comment placement is correct.
- [ ] 8.3 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-module-reserved` from
      `ignored-by-unsupported-features.md`. Run and confirm reserved identifier
      deconfliction is correct (also removes the `@generates es` variant).
- [ ] 8.4 **Rollup fixture (red → green)**: Remove
      `rollup@form@system-semicolon` from `ignored-by-unsupported-features.md`.
      Run and confirm ASI handling in SystemJS output is correct.
- [ ] 8.5 **Rollup fixture (red → green)**: Remove
      `rollup@form@modify-export-semi` from
      `ignored-by-unsupported-features.md`. Run and confirm semicolon insertion
      at export modification sites is correct.
- [ ] 8.6 Implement `_mergeNamespaces` inline helper emission in
      `render_system()` for `export * from 'external'` where multiple external
      namespaces must be merged — emit only when needed, directly into the chunk
- [ ] 8.7 Add `OutputFormat::System` arm to `render_wrapped_entry_chunk` in
      `render_chunk_exports.rs` — handle CJS-wrapped entry modules bundled into
      a SystemJS chunk
- [ ] 8.8 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/star_export_external/`
      with `export * from 'external-lib'` (external). Verify `_starExcludes`,
      setter loop, and `_mergeNamespaces` helper are all emitted correctly.
- [ ] 8.9 **Fixture (red → green)**: Create
      `crates/rolldown/tests/rolldown/function/format/system/tla/` with a module
      using top-level await. Verify `execute: (async function () {` is emitted.

## 9. TLA (Top-Level Await)

- [ ] 9.1 Trace the TLA path for ESM format in rolldown (how does `execute`
      become `async function` currently) — identify where the TLA flag lives
- [ ] 9.2 Implement `async execute` emission in `render_system()` gated on the
      TLA flag from task 9.1
- [ ] 9.3 **Rollup fixture (red → green)**: The
      `rollup@form@top-level-await@generates system` test is currently in
      `ignored-passed-snapshot-different-tests.js`. After implementing TLA,
      remove it from there if the snapshot now matches.

## 10. Final Sweep and Cleanup

- [ ] 10.1 Replace all `todo!()` stubs introduced in task 1.7 with correct
      implementations
- [ ] 10.2 Run full Rollup form+chunking-form test suite:
      `just test-node-rollup` — confirm all 16 previously-skipped SystemJS
      fixtures now pass; update `packages/rollup-tests/src/status.md` and
      `status.json`
- [ ] 10.3 Run full rolldown test suite: `just test-rust` — confirm no
      regressions in existing formats
- [ ] 10.4 Run `just roll` (build + lint + test everything) — confirm clean
- [ ] 10.5 Update `packages/rolldown/src/options/output-options.ts` JSDoc for
      `format` to mention `"system"`
- [ ] 10.6 Update rolldown docs (`docs/`) to list SystemJS as a supported format
- [ ] 10.7 Add `"system"` to `OutputFormat` enum in
      `crates/rolldown_testing/_config.schema.json` so fixtures using
      `"format": "system"` pass JSON schema validation
