# JSON modules

## Parsing and JavaScript strings

`rolldown_common::parse_json_to_ecma_ast` normally parses with `serde_json::Value` and constructs the AST directly. This path does not run the JavaScript parser.

JSON strings can contain escaped lone UTF-16 surrogates, which a Rust `String` cannot represent. When the normal parser fails, a validation copy replaces surrogate escapes with same-length Unicode scalar escapes, then passes through the same `serde_json::Value` parser. This preserves JSON grammar and parser limits, including the nesting limit. Escaped backslashes are skipped and invalid escapes remain invalid. The copy is used only for validation; OXC reads the original source into its `JSStr` representation. If the copy fails validation, the original JSON diagnostic is kept. JavaScript-only syntax never reaches OXC.

Before semantic analysis, the fallback normalizes object properties to JSON semantics:

- Duplicate decoded keys keep the first insertion position and the last value.
- A decoded `__proto__` key is computed, preserving its own data property instead of setting the object's prototype. This also covers escaped spellings and duplicate keys.
- Node spans use the synthetic `SPAN`, matching the directly constructed JSON AST.

The core JSON lazy-export pass handles the resulting expression as usual. Keys containing lone surrogates remain on the default export, since such keys cannot be ECMAScript export names.

The builtin Vite JSON plugin uses the same fallback for its ESM conversion. It emits named bindings only when `namedExports` is enabled and reuses each binding on the default object, keeping object identity intact. In stringify mode it validates the exceptional input and keeps its raw JSON text for the runtime `JSON.parse`, including when minification was requested.

## Files

- `crates/rolldown_common/src/ecmascript/json_to_program.rs` — parsing and AST construction
- `crates/rolldown/src/utils/parse_to_ecma_ast.rs` — JSON diagnostics at the module boundary
- `crates/rolldown/src/stages/link_stage/generate_lazy_export.rs` — default and named exports
- `crates/rolldown_plugin_utils/src/data_to_esm.rs` — Vite plugin ESM conversion
- `crates/rolldown_plugin_vite_json/src/lib.rs` — Vite plugin stringify mode

See [AST construction](../ast-construction/implementation.md) for synthetic node conventions.
