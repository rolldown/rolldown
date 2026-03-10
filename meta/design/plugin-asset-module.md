# Plugin Asset Module

## Summary

Asset modules (`ModuleType::Asset`) are handled by a built-in Rust plugin (`rolldown_plugin_asset_module`) instead of hardcoded logic in the core. The plugin uses existing plugin APIs (`load`, `renderChunk`, `emitFile`) following the same pattern as `CopyModulePlugin`.

**`load` hook (Post order):** Reads the file as binary, emits it via `ctx.emit_file_async()`, associates the module with the emitted file, and returns `module.exports = "__ROLLDOWN_ASSET__#<ref_id>"` as `ModuleType::Js`. The parser auto-detects `module.exports` as CJS.

**`renderChunk` hook (Pre order):** Scans for `__ROLLDOWN_ASSET__#` placeholders using `memchr::memmem`, resolves each ref_id to an asset filename via `ctx.get_file_name()`, and replaces placeholders with relative paths.

**`new URL()` bridge:** The `FileEmitter` has a `module_to_file_ref` map so the `new URL('./asset', import.meta.url)` finalizer can look up the emitted asset by module ID. See [new URL() Design Tradeoff](#new-url-design-tradeoff) for why this is handled in core.

## `has_lazy_export` and CJS Interop

### Background

The old built-in `ModuleType::Asset` was in the `has_lazy_export` list. `has_lazy_export` is a linker-stage mechanism that **defers the wrapping decision** until the linker knows how the module is consumed:

- Module source is a raw expression with no `export`/`module.exports`
- `determine_module_exports_kind` keeps it as `ExportsKind::None` when `import`'d (guarded by `!has_lazy_export()`)
- `require()` can still set it to `ExportsKind::CommonJs`
- `generate_lazy_export` then wraps accordingly:
  - Only `import`'d → `export default expr` (ESM, zero interop overhead)
  - Only `require()`'d → `module.exports = expr` (CJS, returns string directly)
  - Both → CJS wins; ESM consumers pay `__toESM` interop

This is optimal for both CJS and ESM consumers. Plugins cannot replicate this — the plugin must commit to an export style at load time, before the linker knows how the module will be consumed.

### Options Considered

**Option 1: `export default "..."` (ESM) — Rejected**

- ESM imports: optimal, zero wrapping overhead
- `new URL()` only: works (valid ESM syntax when inlined as dead code)
- CJS `require()`: **runtime behavior change** — returns `{ __esModule: true, default: "..." }` instead of the string directly, because `__toCommonJS` wraps the ESM namespace
- Rejected: CJS correctness is important and this breaks `require()` consumers

**Option 2: `module.exports = "..."` (CJS) — Chosen**

- CJS `require()`: correct, `__commonJSMin` returns `module.exports` directly (the string)
- ESM imports: works via `__toESM` interop, adds small wrapping overhead
- `new URL()` only: **safe** — the load hook returns `side_effects: false`, so when nothing imports from the module (only `new URL()` references), tree-shaking excludes the module's statements entirely. The `module.exports` line never appears in output. Without `side_effects: false`, the CJS wrapper would be tree-shaken (nothing calls `require_asset()`) but the bare `module.exports` assignment would remain as a side-effectful top-level statement, causing `ERR_AMBIGUOUS_MODULE_SYNTAX` in ESM contexts.
- Chosen: CJS correctness preserved, ESM works with minor overhead, `new URL()` is safe

**Option 3: `has_lazy_export` via plugin API (future)**

- Would need a new plugin architecture feature — e.g. `LoadOutput::LazyDefaultExportExpr`, a special return type in the Rust load hook that lets the plugin return a raw expression without committing to `export default` or `module.exports`
- The linker would then defer the wrapping decision as before via `generate_lazy_export`
- This would restore optimal zero-overhead ESM imports while keeping CJS correct
- Future consideration to close the remaining ESM overhead gap

### Decision

**Use `module.exports = "..."` (Option 2).** This preserves CJS `require()` correctness — the returned value is the string directly, matching the old built-in behavior. ESM `import` works correctly via `__toESM` interop with a small overhead. The `new URL()` only case is safe because the CJS wrapper is tree-shaken when nothing references it.

The remaining ESM overhead gap (compared to the old `has_lazy_export` approach) can be addressed in the future through Option 3 — a `LoadOutput::LazyDefaultExportExpr` plugin API that defers the wrapping decision to the linker.

## Snapshot Differences from Old Built-in Approach

1. **Hash values** — `FileEmitter` hashing vs old `HashPlaceholderGenerator` pipeline
2. **Chunk-relative paths (bug fix)** — `compute_relative_path()` computes paths relative to the chunk, not the output root. The old code used `preliminary.as_str()` directly (output-root-relative), which was wrong when chunks are in subdirectories (e.g. `entries/entry.js` → `png/image.png` was `"png/image.png"` instead of `"../png/image.png"`). New behavior matches esbuild.
3. **ESM-only imports** — for modules that only `import` an asset (never `require()`), the old `has_lazy_export` inlined directly (`var x = "path"`). The plugin approach uses `__toESM(require_asset())` since `module.exports` makes the module CJS. Functionally identical, small code size overhead.

## `new URL()` Design Tradeoff

`new URL('./asset', import.meta.url)` requires rewriting the first argument to the resolved asset path. Rollup and rolldown handle this differently:

**Rollup:** No core `new URL()` support. A community plugin (`@web/rollup-plugin-import-meta-assets`) uses the `transform` hook to re-parse each module's AST, detect `new URL()` patterns, call `this.emitFile()`, and rewrite to `import.meta.ROLLUP_FILE_URL_<ref>`. Rollup's core then resolves `ROLLUP_FILE_URL_<ref>` during rendering. This is purely plugin-driven but requires **extra AST parsing** in the transform hook.

**Rolldown:** The core scanner detects `new URL()` during the **initial parse** (zero extra parsing cost), creating an import record with `ImportKind::NewUrl`. The core finalizer resolves the asset path via `FileEmitter.file_ref_for_module()` — a bridge API that maps module IDs to emitted file reference IDs. The plugin's `load` hook populates this mapping via `ctx.associate_module_with_file_ref()`.

This is a balanced tradeoff: a small bridge API on the `FileEmitter` (`associate_module_with_file_ref` / `file_ref_for_module`) avoids the cost of re-parsing modules in a plugin `transform` hook. The core handles detection and rewriting; the plugin handles asset emission and filename generation.

## Related

- `crates/rolldown_plugin_asset_module/` — plugin implementation
- `crates/rolldown_plugin_copy_module/` — similar plugin pattern for `ModuleType::Copy`
