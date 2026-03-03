# CJS Cross-Chunk Symbol Resolution Tests

This directory contains tests for verifying correct symbol resolution when accessing symbols from other chunks in CJS output format.

## Test Matrix

Based on the code in `crates/rolldown/src/module_finalizers/mod.rs` (lines 330-354), there are valid combinations to test.

**Note**: Some combinations are invalid:

- `Entry + Esm + Default` is invalid because ESM wrapping adds extra exports (`__esmMin`, `__toESM`, `init_*`) which conflicts with `exports: "default"` requirement (exactly one export)
- `Common + * + Default` is invalid because common chunks always have `OutputExports::Named`

| #   | Chunk Type | WrapKind | OutputExports | Export Name | Expected Result              | Test Coverage                                                                                           |
| --- | ---------- | -------- | ------------- | ----------- | ---------------------------- | ------------------------------------------------------------------------------------------------------- |
| 1   | Entry      | Cjs      | Default       | default     | `require_binding`            | `entry_cjs_default_default` (BUG FOUND)                                                                 |
| 2   | Entry      | Cjs      | Named         | default     | `require_binding.default`    | `entry_cjs_named_default`                                                                               |
| 3   | Entry      | Cjs      | Named         | named       | `require_binding.exportName` | `entry_cjs_named_named`                                                                                 |
| 4   | ~~Entry~~  | ~~Esm~~  | ~~Default~~   | ~~default~~ | ~~N/A~~                      | **INVALID** (ESM wrap adds extra exports)                                                               |
| 5   | Entry      | Esm      | Named         | default     | `require_binding.default`    | `entry_esm_named_default`                                                                               |
| 6   | Entry      | Esm      | Named         | named       | `require_binding.exportName` | `entry_esm_named_named`                                                                                 |
| 7   | Entry      | None     | Default       | default     | `require_binding`            | `cjs_compat/issue_7833`                                                                                 |
| 8   | Entry      | None     | Named         | default     | `require_binding.default`    | `entry_none_named_default`                                                                              |
| 9   | Entry      | None     | Named         | named       | `require_binding.exportName` | `entry_none_named_named`                                                                                |
| 10  | Common     | Cjs      | Named         | default     | `require_binding.default`    | `common_cjs_named_default`                                                                              |
| 11  | Common     | Cjs      | Named         | named       | `require_binding.exportName` | `optimization/chunk_merging/dynamic_entry_merged_in_common_chunk`                                       |
| 12  | Common     | Esm      | Named         | default     | `require_binding.default`    | `topics/live_bindings/default_export_binding_in_common_chunks_cjs`                                      |
| 13  | Common     | Esm      | Named         | named       | `require_binding.exportName` | `common_esm_named_named`                                                                                |
| 14  | Common     | None     | Named         | default     | `require_binding.default`    | `topics/live_bindings/default_export_expr_in_common_chunks_cjs`                                         |
| 15  | Common     | None     | Named         | named       | `require_binding.exportName` | `topics/generated_code/symbols_common_chunk`, `topics/live_bindings/named_exports_in_common_chunks_cjs` |

## Bugs Found

### Test #1: Entry + Cjs + Default + default

The generated output has a bug where `lib.js` generates TWO `module.exports =` statements:

1. First exports the runtime helpers (`__toESM`, `__toCommonJS`, `__name`)
2. Second exports the actual value (overwriting the first)

This causes `main.js` to fail at runtime with `require_lib$1.__toESM is not a function` because the runtime helpers are overwritten.

## Test Naming Convention

Tests in this directory follow the naming pattern:
`{chunk_type}_{wrap_kind}_{output_exports}_{export_name}/`

Examples:

- `entry_cjs_default_default/` - Entry chunk, CJS module, Default exports, accessing default
- `common_esm_named_named/` - Common chunk, ESM module with strict execution order, Named exports, accessing named export
