# Replacing `stable_id` with Absolute ID (`module.id`) in HMR Runtime

## Problem

The HMR runtime currently uses `stable_id` (relative paths like `src/module.js`) for module registration and lookup, but lazy compilation uses absolute paths (`module.id`). This causes `loadExports()` to fail because the keys don't match.

## Solution

Change all HMR runtime code to use **absolute paths** (`module.id`) instead of `stable_id` for consistency.

## Files to Modify

### 1. `crates/rolldown/src/hmr/utils.rs`

**Function `generate_hmr_init_code`** (around line 20-40):

Change `registerModule()` call:

```rust
// FROM:
"__rolldown_runtime__.registerModule(\"{}\", {{ exports: {} }});",
module.stable_id,

// TO:
"__rolldown_runtime__.registerModule(\"{}\", {{ exports: {} }});",
module.id,
```

Change `createModuleHotContext()` call:

```rust
// FROM:
"const {} = __rolldown_runtime__.createModuleHotContext(\"{}\");",
hot_context_name,
module.stable_id,

// TO:
"const {} = __rolldown_runtime__.createModuleHotContext(\"{}\");",
hot_context_name,
module.id,
```

### 2. `crates/rolldown/src/hmr/hmr_ast_finalizer.rs`

**Function `rewrite_dynamic_import`** (around line 200-250):

Change `loadExports()` call in dynamic import rewriting:

```rust
// FROM:
let load_exports_call = self.snippet.call_expr_with_arg_expr_and_paren(
  self.snippet.id_ref_expr("__rolldown_runtime__.loadExports", SPAN),
  self.builder.expression_string_literal(SPAN, importee.stable_id.as_str(), None),
);

// TO:
let load_exports_call = self.snippet.call_expr_with_arg_expr_and_paren(
  self.snippet.id_ref_expr("__rolldown_runtime__.loadExports", SPAN),
  self.builder.expression_string_literal(SPAN, importee.id.as_str(), None),
);
```

**Function `rewrite_hot_accept_call`** (around line 300-350):

Change `import.meta.hot.accept()` dependency specifiers:

```rust
// FROM: uses stable_id for the specifier
// TO: use importee.id (absolute path)
```

### 3. `crates/rolldown/src/hmr/hmr_stage.rs`

**Function `compute_updated_modules`** (around line 150-200):

Change `HmrBoundaryOutput` boundaries to use absolute IDs:

```rust
// FROM:
boundaries.push(module.stable_id.to_string());

// TO:
boundaries.push(module.id.to_string());
```

**In `applyUpdates()` call generation**:

```rust
// FROM:
let boundaries_json: Vec<_> = boundaries.iter().map(|b| format!("\"{}\"", b)).collect();

// TO: (same, but boundaries now contain absolute IDs)
```

### 4. `crates/rolldown/src/module_finalizers/hmr.rs`

**Function `rewrite_hot_accept_call_deps`** in `ScopeHoistingFinalizer`:

Change the specifier used for `import.meta.hot.accept()`:

```rust
// FROM:
self.builder.expression_string_literal(SPAN, importee.stable_id.as_str(), None)

// TO:
self.builder.expression_string_literal(SPAN, importee.id.as_str(), None)
```

## Test File to Update

### `crates/rolldown/tests/rolldown/topics/hmr/register_exports/_test.mjs`

The test finds modules by ID. Update to find by absolute path suffix:

```javascript
// FROM:
const sharedModule = Object.entries(modules).find(([key]) =>
  key === 'src/shared.js'
);

// TO:
const sharedModule = Object.entries(modules).find(([key]) =>
  key.endsWith('src/shared.js')
);
```

## Verification

After making changes:

1. Run `just test-rust` - all HMR tests should pass
2. Run `just build-rolldown`
3. Test lazy compilation example: `cd examples/lazy && pnpm dev`
4. Verify `loadExports()` calls use absolute paths matching `registerModule()` calls

## Key Insight

The runtime stores modules in a map keyed by module ID. For `loadExports(id)` to find a module, the `id` must match exactly what was used in `registerModule(id, ...)`. Using absolute paths everywhere ensures consistency between:

- Initial module registration
- Dynamic import `loadExports()` calls
- HMR boundary tracking
- `import.meta.hot.accept()` dependency resolution
