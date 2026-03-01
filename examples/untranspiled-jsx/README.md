# UntranspiledSyntaxError Investigation

## Issue

[#8216](https://github.com/rolldown/rolldown/issues/8216) reported a panic (`unreachable!(): jsx should be transpiled`) in `side_effect_detector/mod.rs` when untranspiled JSX was encountered during bundling.

The rolldown team:

1. Replaced the `unreachable!()` with conservative side-effect treatment (JSX expressions treated as side-effectful)
2. Added `UntranspiledSyntaxError` diagnostic in the scanner to catch untranspiled JSX/TS

## Findings

### The panic is no longer reproducible

The `unreachable!()` at `side_effect_detector/mod.rs` has been replaced. JSX/TS expressions are now handled at `side_effect_detector/mod.rs:569-583` as conservatively side-effectful.

### `UntranspiledSyntaxError` cannot be triggered from the JS API

The diagnostic is emitted in `impl_visit.rs:141-146` when the scanner encounters JSX nodes AND `FlatOptions::JsxPreserve` is `false`.

However, there is a **chicken-and-egg coupling** in the code:

- `FlatOptions::JsxPreserve` is set from `preserve_jsx` in `pre_process_ecma_ast.rs:116-117`, which is `true` when `transform_options.jsx.jsx_plugin` is `false`.
- When `jsx_plugin` is `true` → JSX gets transpiled by the oxc transformer → no JSX nodes survive to the scanner → no error.
- When `jsx_plugin` is `false` → `preserve_jsx = true` → `FlatOptions::JsxPreserve = true` → scanner allows JSX → no error.

**There is no config combination that results in JSX nodes in the AST AND `jsx_preserve() = false`.**

The `jsxPreset` debug-only override (`#[cfg(debug_assertions)]`) changes `JsxPreset` on `TransformOptions`, but `FlatOptions::JsxPreserve` is derived from a completely different source (`transform_options.jsx.jsx_plugin`), so the override has no effect on triggering the error.

### Code path analysis

```
parse_to_ecma_ast (parse → transformAst plugin hook → pre_process_ecma_ast)
  └─ pre_process_ecma_ast.rs:104-118
       preserve_jsx = false
       if is_not_js || should_transform_js() || contains_script_closing_tag():
         run transformer
         if !transform_options.jsx.jsx_plugin:
           preserve_jsx = true    ← always set when JSX is NOT transpiled

create_ecma_view
  └─ ecma_module_view_factory.rs:31
       FlatOptions::JsxPreserve = preserve_jsx   ← from above

scanner (impl_visit.rs:512-526)
  └─ visit_jsx_element / visit_jsx_fragment:
       if jsx_preserve():   ← checks FlatOptions::JsxPreserve
         walk JSX (allow)
       else:
         flag UntranspiledSyntax::Jsx   ← UNREACHABLE in normal pipeline
```

### When it would trigger

The error was designed for **Vite-like integration scenarios** where:

- Files are scanned/analyzed without running transform plugins (e.g., SSR dep optimization)
- JSX files get parsed but transform plugins (like `@vitejs/plugin-react`) haven't processed them
- These scenarios are not reachable through the standard rolldown JS/Rust bundler API

### Tested approaches (all failed to trigger the error)

| #   | Approach                                                       | Result                   |
| --- | -------------------------------------------------------------- | ------------------------ |
| 1   | `jsx: "preserve"` + `jsxPreset: "enable"`                      | JSX preserved, no error  |
| 2   | Plugin `load` returning JSX with `moduleType: "jsx"`           | JSX transpiled, no error |
| 3   | Plugin `load` + `jsx: "preserve"` + `jsxPreset: "enable"`      | JSX preserved, no error  |
| 4   | Plugin `transform` returning JSX with `moduleType: "jsx"`      | JSX transpiled, no error |
| 5   | Plugin `transform` + `jsx: "preserve"`                         | JSX preserved, no error  |
| 6   | Plugin `transform` + `jsx: "preserve"` + `jsxPreset: "enable"` | JSX preserved, no error  |

## Conclusion

The `UntranspiledSyntaxError` code path at `impl_visit.rs:141-146` is effectively **dead code** in the normal bundler pipeline. To make it triggerable for testing purposes, the `FlatOptions::JsxPreserve` flag would need to be decoupled from `preserve_jsx` (which is derived from `transform_options.jsx.jsx_plugin`), or a separate mechanism would be needed to simulate the Vite SSR scan scenario.
