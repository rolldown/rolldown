# Const Enum Rolldown Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable cross-module const enum inlining in rolldown by consuming pre-computed enum member values from oxc_semantic.

**Architecture:** During `pre_process_ecma_ast`, enum member values are extracted from the initial `Scoping` (before the transformer runs). The transformer emits `var X = {}` placeholders for const enums. The extracted values are passed through `ParseToEcmaAstResult` to `AstScanner`, which populates `constant_export_map`. Rolldown's existing cross-module inlining pipeline handles the rest. The finalizer removes const enum placeholders.

**Tech Stack:** Rust, rolldown, oxc (oxc_syntax, oxc_semantic, oxc_transformer)

**Spec:** `../oxc/docs/superpowers/specs/2026-03-18-const-enum-support-design.md` (Section 5)

**Depends on:** oxc `feat/const-enum-support` branch (ConstantValue type, Scoping.enum_member_values, transformer optimize_const_enums + emit_const_enum_placeholder options)

---

## File Map

| File                                                                                               | Action | Responsibility                                                                                                                |
| -------------------------------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `crates/rolldown/src/utils/parse_to_ecma_ast.rs:36-44`                                             | Modify | Add `enum_member_values` and `const_enum_names` fields to `ParseToEcmaAstResult`                                              |
| `crates/rolldown/src/utils/pre_process_ecma_ast.rs:61-182`                                         | Modify | Extract enum values from step 1 Scoping, enable `optimize_const_enums` + `emit_const_enum_placeholder` in transformer options |
| `crates/rolldown_common/src/inner_bundler_options/types/transform_option/typescript_options.rs:57` | Modify | Wire `optimize_const_enums` and `emit_const_enum_placeholder` through options                                                 |
| `crates/rolldown/src/ecmascript/ecma_module_view_factory.rs:39-78`                                 | Modify | Pass `enum_member_values` and `const_enum_names` to AstScanner                                                                |
| `crates/rolldown/src/ast_scanner/mod.rs:64-260`                                                    | Modify | Accept enum values, populate `constant_export_map` from them                                                                  |
| `crates/rolldown_common/src/types/symbol_ref_db.rs:25-58`                                          | Modify | Add `ConstEnumPlaceholder` flag to `SymbolRefFlags`                                                                           |
| `crates/rolldown/src/module_finalizers/scope_hoisting/mod.rs`                                      | Modify | Remove const enum placeholder declarations during finalization                                                                |

---

### Task 1: Add enum value fields to `ParseToEcmaAstResult`

**Files:**

- Modify: `crates/rolldown/src/utils/parse_to_ecma_ast.rs:36-44`

- [ ] **Step 1: Add fields to the struct**

In `crates/rolldown/src/utils/parse_to_ecma_ast.rs`, add to `ParseToEcmaAstResult`:

```rust
use oxc::syntax::constant_value::ConstantValue;
use compact_str::CompactStr;
use rustc_hash::{FxHashMap, FxHashSet};

pub struct ParseToEcmaAstResult {
    pub ast: EcmaAst,
    pub scoping: Scoping,
    pub has_lazy_export: bool,
    pub warnings: Vec<BuildDiagnostic>,
    pub preserve_jsx: bool,
    /// Enum member constant values extracted from semantic analysis.
    /// Keyed by enum declaration name → Vec of (member_name, value).
    pub enum_member_values: FxHashMap<CompactStr, Vec<(CompactStr, ConstantValue)>>,
    /// Names of const enum declarations (for placeholder removal in finalizer).
    pub const_enum_names: FxHashSet<CompactStr>,
}
```

- [ ] **Step 2: Update all construction sites**

Find every place that creates `ParseToEcmaAstResult`. The main one is in `pre_process_ecma_ast.rs` line 182:

```rust
Ok(ParseToEcmaAstResult { ast, scoping, has_lazy_export, warnings, preserve_jsx })
```

Add the new fields with empty defaults for now:

```rust
Ok(ParseToEcmaAstResult {
    ast, scoping, has_lazy_export, warnings, preserve_jsx,
    enum_member_values: FxHashMap::default(),
    const_enum_names: FxHashSet::default(),
})
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p rolldown`

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown/src/utils/parse_to_ecma_ast.rs
git commit -m "feat: add enum_member_values and const_enum_names to ParseToEcmaAstResult"
```

---

### Task 2: Extract enum values from initial Scoping

**Files:**

- Modify: `crates/rolldown/src/utils/pre_process_ecma_ast.rs:61-182`

This is the core pipeline change. After step 1 (SemanticBuilder) and before step 3 (Transformer), extract enum member values from Scoping.

- [ ] **Step 1: Add extraction logic after step 1**

After line 88 (`let mut scoping = Some(semantic_ret.semantic.into_scoping());`), add:

```rust
    // Extract enum member values before the transformer runs.
    // The transformer will convert enums to IIFEs / placeholders, after which
    // recreate_scoping() would lose the enum member data.
    let (enum_member_values, const_enum_names) = {
        let scoping_ref = scoping.as_mut().unwrap();
        let raw_values = scoping_ref.take_enum_member_values();

        let mut enum_values: FxHashMap<CompactStr, Vec<(CompactStr, ConstantValue)>> = FxHashMap::default();
        let mut const_names: FxHashSet<CompactStr> = FxHashSet::default();

        // Group member values by their parent enum declaration.
        // Iterate all symbols to find enum members and their parent enum.
        for symbol_id in scoping_ref.symbol_ids() {
            let flags = scoping_ref.symbol_flags(symbol_id);
            if !flags.is_enum_member() {
                continue;
            }
            if let Some(value) = raw_values.get(&symbol_id) {
                let member_name = CompactStr::from(scoping_ref.symbol_name(symbol_id));
                // Find the parent enum by looking at the member's scope's parent scope's bindings
                let member_scope = scoping_ref.symbol_scope_id(symbol_id);
                if let Some(parent_scope) = scoping_ref.get_parent_id(member_scope) {
                    // Find which symbol in the parent scope owns this enum body scope
                    for parent_symbol_id in scoping_ref.get_bindings(parent_scope).values() {
                        let parent_flags = scoping_ref.symbol_flags(*parent_symbol_id);
                        if parent_flags.is_const_enum() || parent_flags.is_regular_enum() {
                            let enum_name = CompactStr::from(scoping_ref.symbol_name(*parent_symbol_id));
                            if parent_flags.is_const_enum() {
                                const_names.insert(enum_name.clone());
                            }
                            enum_values.entry(enum_name).or_default().push((member_name.clone(), value.clone()));
                            break;
                        }
                    }
                }
            }
        }

        (enum_values, const_names)
    };
```

NOTE: The exact API calls (`symbol_ids()`, `symbol_flags()`, `symbol_name()`, `symbol_scope_id()`, `get_parent_id()`, `get_bindings()`) need to be verified against the actual `Scoping` API. Read `oxc_semantic/src/scoping.rs` to confirm method signatures. Some methods may be on `ScopeTable` or accessed differently.

- [ ] **Step 2: Pass enum values to the return struct**

Update line 182 to include the extracted values:

```rust
Ok(ParseToEcmaAstResult {
    ast, scoping, has_lazy_export, warnings, preserve_jsx,
    enum_member_values,
    const_enum_names,
})
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p rolldown`

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown/src/utils/pre_process_ecma_ast.rs
git commit -m "feat: extract enum member values from initial Scoping before transformer"
```

---

### Task 3: Wire `optimize_const_enums` and `emit_const_enum_placeholder` through transformer options

**Files:**

- Modify: `crates/rolldown_common/src/inner_bundler_options/types/transform_option/typescript_options.rs:57`

- [ ] **Step 1: Enable the options in the conversion**

At line 57 of `typescript_options.rs`, change:

```rust
optimize_const_enums: false,
```

to:

```rust
optimize_const_enums: true,
emit_const_enum_placeholder: true,
```

This tells the oxc transformer to:

- Convert const enum declarations to `var X = {}` placeholders (instead of IIFEs)
- Skip same-file reference inlining (rolldown handles cross-module inlining)

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p rolldown`

Note: This requires the oxc dependency to include the new `emit_const_enum_placeholder` field. If the oxc dependency hasn't been updated yet, this step will fail. In that case, bump the oxc dependency first.

- [ ] **Step 3: Commit**

```bash
git add crates/rolldown_common/src/inner_bundler_options/types/transform_option/typescript_options.rs
git commit -m "feat: enable optimize_const_enums and emit_const_enum_placeholder for rolldown"
```

---

### Task 4: Pass enum values to AstScanner and populate `constant_export_map`

**Files:**

- Modify: `crates/rolldown/src/ast_scanner/mod.rs:146-260`
- Modify: `crates/rolldown/src/ecmascript/ecma_module_view_factory.rs:39-78`

- [ ] **Step 1: Add enum values parameter to AstScanner**

In `crates/rolldown/src/ast_scanner/mod.rs`, add a new parameter to `AstScanner::new()` (line 172):

```rust
pub fn new(
    module_idx: ModuleIdx,
    scoping: Scoping,
    // ... existing params ...
    enum_member_values: &FxHashMap<CompactStr, Vec<(CompactStr, ConstantValue)>>,
    const_enum_names: &FxHashSet<CompactStr>,
) -> Self {
```

At the end of `new()`, before returning `Self { ... }`, populate `constant_export_map` from enum values:

```rust
    // Populate constant_export_map from pre-computed enum member values.
    for (enum_name, members) in enum_member_values {
        // Find the enum declaration's symbol in the new Scoping
        // (it's a `var X = {}` placeholder after transformation)
        let root_scope_id = scoping.root_scope_id();
        if let Some(enum_symbol_id) = scoping.get_binding(root_scope_id, enum_name.as_str().into()) {
            for (member_name, value) in members {
                // The member symbols don't exist in the post-transform Scoping.
                // Store the constant values keyed by a synthetic approach or
                // integrate into the existing constant_export_map via the enum's symbol.
                let rolldown_value = match value {
                    ConstantValue::Number(n) => rolldown_common::ConstantValue::Number(*n),
                    ConstantValue::String(s) => rolldown_common::ConstantValue::String(s.clone()),
                };
                // For enum member inlining, we need the members accessible via
                // the enum namespace. Store in a separate enum_constants map.
                // (See Step 2 for the data structure.)
            }
        }
    }
```

NOTE: The exact mechanism for how enum member values map to rolldown's inlining depends on how `Direction.Up` is resolved. In rolldown, `import { Direction } from './a'; Direction.Up` is a namespace alias access. The `try_inline_constant_from_namespace_alias` method (mod.rs:429) resolves the property name against `constant_export_map`. For this to work, each enum member needs its own entry in `constant_export_map`. But enum member symbols don't exist in the rebuilt Scoping.

Alternative approach: add an `enum_member_constants: FxHashMap<SymbolId, FxHashMap<CompactStr, ConstantValue>>` to `ScanResult` keyed by the enum declaration's SymbolId. The finalizer's namespace alias resolver can then check this map when the property name matches.

This is the trickiest integration point. Read `try_inline_constant_from_namespace_alias` carefully to understand how it resolves property names. The key line is the `resolved_exports.get(&namespace_alias.property_name)` lookup — this means the property name needs to be in the module's resolved exports as a named export.

- [ ] **Step 2: Pass enum values from `ecma_module_view_factory.rs`**

In `crates/rolldown/src/ecmascript/ecma_module_view_factory.rs`, update the `AstScanner::new()` call (lines 39-50) to pass the enum values:

```rust
let ParseToEcmaAstResult {
    ast, scoping, has_lazy_export, warnings: parse_warnings, preserve_jsx,
    enum_member_values, const_enum_names,
} = parse_to_ecma_ast(ctx, source).await?;

// ... existing code ...

let scanner = AstScanner::new(
    ctx.module_idx,
    scoping,
    &repr_name,
    ctx.resolved_id.module_def_format,
    ast.source(),
    &module_id,
    ast.comments(),
    ctx.options,
    ast.allocator(),
    ctx.flat_options,
    &enum_member_values,
    &const_enum_names,
);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p rolldown`

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown/src/ast_scanner/mod.rs crates/rolldown/src/ecmascript/ecma_module_view_factory.rs
git commit -m "feat: pass enum member values to AstScanner and populate constant_export_map"
```

---

### Task 5: Add `ConstEnumPlaceholder` flag to `SymbolRefFlags`

**Files:**

- Modify: `crates/rolldown_common/src/types/symbol_ref_db.rs:25-58`

- [ ] **Step 1: Add the flag**

In `crates/rolldown_common/src/types/symbol_ref_db.rs`, add to the `SymbolRefFlags` bitflags (line 56):

```rust
    const ConstEnumPlaceholder = 1 << 7;
```

- [ ] **Step 2: Mark const enum symbols in AstScanner**

Back in `crates/rolldown/src/ast_scanner/mod.rs`, after populating the constant_export_map for enum members, mark the enum declaration symbol:

```rust
if const_enum_names.contains(enum_name) {
    self.result.symbol_ref_db.flags.entry(enum_symbol_id)
        .or_default()
        .insert(SymbolRefFlags::ConstEnumPlaceholder);
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p rolldown`

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown_common/src/types/symbol_ref_db.rs crates/rolldown/src/ast_scanner/mod.rs
git commit -m "feat: add ConstEnumPlaceholder flag for const enum removal in finalizer"
```

---

### Task 6: Remove const enum placeholders in finalizer

**Files:**

- Modify: `crates/rolldown/src/module_finalizers/scope_hoisting/mod.rs`

- [ ] **Step 1: Find where variable declarations are emitted**

Read the scope hoisting finalizer code. Look for where `VariableDeclaration` statements are visited or emitted. The finalizer needs to check if the declared variable has `SymbolRefFlags::ConstEnumPlaceholder` and skip it if so.

- [ ] **Step 2: Add placeholder removal logic**

In the visitor that processes statements or variable declarations, add:

```rust
// Check if this variable declaration is a const enum placeholder
if let Declaration::VariableDeclaration(var_decl) = &stmt.declaration {
    if var_decl.declarations.len() == 1 {
        if let Some(BindingPatternKind::BindingIdentifier(ident)) =
            var_decl.declarations[0].id.kind.as_binding_identifier()
        {
            let symbol_id = ident.symbol_id.get().unwrap();
            let symbol_ref = SymbolRef::from((self.ctx.idx, symbol_id));
            if self.ctx.symbol_db.get_flags(symbol_ref)
                .contains(SymbolRefFlags::ConstEnumPlaceholder)
            {
                // Skip this statement — it's a const enum placeholder
                continue; // or return without emitting
            }
        }
    }
}
```

The exact integration depends on how the finalizer's visitor is structured. Read the file to find the right hook point.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p rolldown`

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown/src/module_finalizers/scope_hoisting/mod.rs
git commit -m "feat: remove const enum placeholder declarations in finalizer"
```

---

### Task 7: Add tests and validate

**Files:**

- Create or modify test fixtures in `crates/rolldown/tests/`

- [ ] **Step 1: Find existing const enum test fixtures**

Check `crates/rolldown/tests/esbuild/ts/` for existing const enum tests:

- `ts_const_enum_comments/`
- `ts_enum_cross_module_inlining_access/`
- `ts_enum_cross_module_inlining_definitions/`
- `ts_enum_same_module_inlining_access/`

Run these tests to see current behavior:

```bash
cargo test -p rolldown -- ts_const_enum
cargo test -p rolldown -- ts_enum_cross_module_inlining
```

- [ ] **Step 2: Update snapshots if tests improve**

If the const enum tests now produce better output (values inlined), update snapshots:

```bash
cargo insta review
```

- [ ] **Step 3: Run full rolldown test suite**

```bash
cargo test -p rolldown
```

- [ ] **Step 4: Commit any snapshot updates**

```bash
git add -A
git commit -m "test: update const enum test snapshots with inlined values"
```

---

### Task 8: Final validation

- [ ] **Step 1: Run clippy**

```bash
cargo clippy -p rolldown -p rolldown_common -- -D warnings
```

- [ ] **Step 2: Run full test suite**

```bash
cargo test
```

- [ ] **Step 3: Manual smoke test**

Create a test file:

```typescript
// a.ts
export const enum Direction {
  Up = 0,
  Down = 1,
  Left = 2,
  Right = 3,
}

// b.ts
import { Direction } from './a';
console.log(Direction.Up);
```

Bundle with rolldown and verify `Direction.Up` is inlined to `0` in the output.
