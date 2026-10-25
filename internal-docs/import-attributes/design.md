# Import Attributes — Design & Principles

## Summary

With `experimental.importAttributes`, the import attributes of an import (`with { type: 'json' }`) become part of the module identity, so `import a from './x' with { type: 'json' }` and `import b from './x'` are two different modules. The contract follows Rollup 5 ([rollup#6266](https://github.com/rollup/rollup/pull/6266)) and is specified in the RFC [rolldown#11217](https://github.com/rolldown/rolldown/discussions/11217). For the machinery that realizes it, see [implementation.md](./implementation.md).

## Design principles / Goals

1. **Rollup-compatible ids.** A module with attributes has the id `rawId?attributes=<encodeURIComponent(JSON.stringify(sortedAttributes))>` (`&` when `rawId` already has a `?`). The encoding is byte-identical to Rollup's `generateIdByRawIdAndAttributes`, so plugins that compose or parse ids themselves, and caches keyed by id, see the same strings in both bundlers.
2. **The id is the only key.** The attributes are folded into the id string instead of being a second key beside it. Every map keyed by `ModuleId` (module graph, plugin module info, caches, HMR) then distinguishes the variants for free, and nothing has to be taught about attributes to stay correct.
3. **The file is the raw id.** Reading, watching, tsconfig lookup, extension-based module type and file names use `rawId`. Only identity uses the composed id. Diagnostics keep the composed id, so the user can tell the variants apart.
4. **Plugins decide the final attributes.** `resolveId` gets the requested attributes and may return `attributes` to override them (`{}` drops them). What `resolveId` returns is the module's identity.
5. **Opt-in, no behavior change when off.** Without the option, ids, outputs and the `with` clauses of externals are exactly as before. The new hook fields are filled anyway, so plugins can be written against them before turning the option on. `resolveId` still receives the import's attributes; `load` / `transform` see `{}`. The `INVALID_IMPORT_ATTRIBUTE` warnings also need the option. Without it no attribute takes part in identity, and an `assert` clause is still kept in the output, so there is nothing to warn about.
6. **Module type precedence:** the load hook's `moduleType`, then `attributes.type` (`json` → Json, `text` → Text, `bytes` → Binary), then the file extension and `moduleTypes`, including the `asset` and `copy` types that builtin plugins handle.
7. **A sorted slice, not a map.** `ImportAttributes` is `Option<Arc<[(CompactStr, CompactStr)]>>`. An import has one or two attributes, so a linear scan beats hashing. Keeping them in the JS enumeration order makes the derived `Eq` / `Hash` independent of the written order and turns id composition into a plain loop; a `BTreeMap` (byte order) or an `IndexMap` (insertion order) would need a custom comparator anyway. `None` is the common no-attributes case and allocates nothing. `Arc` because the value is cloned into `ResolvedId`, import records, externals and hook arguments across threads, and `CompactStr` keeps keys like `type` and `json` inline.

## Unresolved Questions

- A plugin returning an already-composed id from `resolveId` without `attributes` gets the requested attributes appended a second time. Should `resolveId` output be parsed with `parseModuleId` first?
- Formats other than ESM have no way to express attributes on externals; nothing warns about the dropped attributes.
- Dev engine / HMR and Vite integration are untouched by the PoC.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- [module-id](../module-id/implementation.md) — `ModuleId` representation
