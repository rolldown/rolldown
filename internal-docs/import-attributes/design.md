# Import Attributes — Design & Principles

## Summary

With `experimental.importAttributes`, the import attributes of an import (`with { type: 'json' }`) become part of the module identity, so `import a from './x' with { type: 'json' }` and `import b from './x'` are two different modules. The contract follows Rollup 5 ([rollup#6266](https://github.com/rollup/rollup/pull/6266)) and is specified in the RFC [rolldown#11217](https://github.com/rolldown/rolldown/discussions/11217). For the machinery that realizes it, see [implementation.md](./implementation.md).

## Design principles / Goals

1. **Rollup-compatible ids.** A module with attributes has the id `rawId?attributes=<encodeURIComponent(JSON.stringify(sortedAttributes))>` (`&` when `rawId` already has a `?`). The encoding is byte-identical to Rollup's `generateIdByRawIdAndAttributes`, so plugins that compose or parse ids themselves, and caches keyed by id, see the same strings in both bundlers.
2. **The id is the only key.** The attributes are folded into the id string instead of being a second key beside it. Every map keyed by `ModuleId` (module graph, plugin module info, caches, HMR) then distinguishes the variants for free, and nothing has to be taught about attributes to stay correct.
3. **The file is the raw id.** Reading, watching, tsconfig lookup, extension-based module type and file names use `rawId`. Only identity uses the composed id. Diagnostics keep the composed id, so the user can tell the variants apart.
4. **Plugins decide the final attributes.** With the option on, `resolveId` gets the requested attributes and may return `attributes` to override them (`{}` drops them). What `resolveId` returns is the module's identity.
5. **Opt-in for internal modules, always on for externals.** Without the option, plugins neither see nor change attributes. Every hook gets `{}`, the attributes a `resolveId` hook returns are ignored, and ids and module types of internal modules behave as before. The fields exist in both modes, so plugins can be written against them before turning the option on. The attributes stay out of plugins because the option is experimental. If plugins could already read them by default, they would come to depend on them, and changing their meaning later would break builds that never opted in. Externals carry their attributes in their id in both modes, because the output must keep the `with` clause. Node needs `type: 'json'` to import JSON, and Rollup 5 keeps the clause too. The earlier way of keeping the clause (#5794, #9796, #11100) had no identity. Two imports of one external with different attributes were one module, and only one clause survived, so `import 'ext'; import 'ext' with { type: 'json' }` rendered only the second import. Now they are two externals, each rendering its own attributes, the ones written in the source when the option is off. The legacy `assert` keyword is treated as `with` in either mode and warns `DEPRECATED_IMPORT_ASSERTION`.
6. **Module type precedence:** the load hook's `moduleType`, then `attributes.type` (`json` → Json, `text` → Text, `bytes` → Binary), then the file extension and `moduleTypes`, including the `asset` and `copy` types that builtin plugins handle.
7. **A sorted slice, not a map.** `ImportAttributes` is `Option<Arc<[(CompactStr, CompactStr)]>>`. An import has one or two attributes, so a linear scan beats hashing. Keeping them in the JS enumeration order makes the derived `Eq` / `Hash` independent of the written order and turns id composition into a plain loop; a `BTreeMap` (byte order) or an `IndexMap` (insertion order) would need a custom comparator anyway. `None` is the common no-attributes case and allocates nothing. `Arc` because the value is cloned into `ResolvedId`, import records, externals and hook arguments across threads, and `CompactStr` keeps keys like `type` and `json` inline.
8. **One warning code per meaning.** `INVALID_IMPORT_ATTRIBUTE` is Rollup's code for `import()` options it cannot analyze, so Rolldown uses it only for those cases. The legacy `assert` keyword gets its own code, `DEPRECATED_IMPORT_ASSERTION`, with its own `checks.*` switch.

## Unresolved Questions

- A plugin returning an already-composed id from `resolveId` without `attributes` gets the requested attributes appended a second time. Without requested attributes, the composed id stays opaque, so its file is read from the whole string and fails. `this.emitFile({ type: 'chunk', id })` with a composed id has the same problem. Should such ids be parsed with `parseModuleId` first?
- In formats other than ESM, a static import of an external becomes `require()` or a global, which cannot carry attributes, and nothing warns when they are dropped. An `import()` with options stays an `import()`.
- Dev engine / HMR and Vite integration are not covered yet. With the option on, lazy compilation skips an `import()` with attributes, so it is bundled as usual. Without it, the plugin sees no attributes and compiles such an import lazily, which is safe because the attributes of an internal module are dropped then.
- With the option on, some code still treats the composed id as a file path. They should use the raw id.
  - `rolldown_plugin_vite_resolve` checks the prefixes of its virtual ids (optional peer deps, browser externals) on `args.id` in `load` and returns no module type, so `type: 'json'` makes the stub fail to parse as JSON.
  - `rolldown_plugin_vite_import_glob` compares the composed id to skip the importer itself, so a glob can include the importing file.
  - `rolldown_plugin_vite_transform` takes the extension and the oxc `Transformer` path from `args.id`.
  - `rolldown_plugin_isolated_declaration` names the emitted `.d.ts` after the composed `stable_id`, so two variants of one file emit the same name.
  - The declaration file checks in `chunk_optimizer.rs` and `inline_common_chunks/exclude.rs` test the composed id.
  - `rolldown_plugin_lazy_compilation` only checks the requested attributes. When a `resolveId` hook adds attributes, the proxy imports a composed id that resolves to an opaque module.
- `rolldown_plugin_vite_manifest` keys a chunk by `relative_path`, which drops the attributes. Two chunks of one file with different attributes share a key, and one is lost. Vite's JS version keys by the full `facadeModuleId`. Should the key keep the suffix?
- Sourcemap `sources` keep the `?attributes=` suffix (`normal_module.rs`, `normalize_transform_sourcemap`). This keeps two variants of one file apart, but tools see a path that does not exist. Keep or strip it?
- Builtin plugins called from JS (`binding_callable_builtin_plugin.rs`) get `{}` as `resolveId` attributes, and the attributes they return are dropped, because `BindingHookJsResolveIdOptions` and `BindingHookJsResolveIdOutput` have no field for them.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- [module-id](../module-id/implementation.md) — `ModuleId` representation
