# Runtime Helpers

## `__commonJS` and `__commonJSMin`: release `cb` after initialization

After the first call in either helper, `mod` is set and `cb` is never accessed again. Without an explicit `cb = null`, the factory is permanently retained in the closure — a memory leak in long-lived processes (e.g. SSR servers loading bundles via `vm.createContext`).

Reference: https://github.com/rolldown/rolldown/issues/9063

## `__toESM`: deciding interop for external modules

`require("external")` returns the raw CommonJS exports, so non-ESM formats must run it through `__toESM` whenever the bundle reads it _as an ES module_ — `import * as ns`, or `ns.default`. A named-only import reads the CommonJS object directly and must not be wrapped.

The trap is that the question cannot be answered from the static imports a chunk's own modules carry. `import d from 'external'; export { d }` links the shim's symbol to a `NamespaceAlias`, so tree-shaking follows the alias straight to the external namespace and never enqueues the shim's declaring statement. With a side-effect-free external the shim is dropped entirely, while the `<external_ns>.default` reference it produced survives elsewhere. Deriving `needs_interop` from `chunk.direct_imports_from_external_modules` then answers "no" for a chunk the finalizer still renders `.default` against — a silently wrong bundle (issue #10069).

So the **inclusion pass records it** (`note_external_interop_use`), being the one place that walks the reference _after_ linking resolved it. Three consumers OR that record into their own `named_imports`-derived answer: the cjs renderer, the iife/umd renderer, and the chunk deconflicter's mixed-mode naming.

Two things follow from the importer being potentially dead:

- **Keeping the helper alive.** `RuntimeHelper::ToEsm` is normally requested by the import statement, so it dies with the module holding it. `include_statements` re-adds it whenever any interop use was recorded, and `patch_module_dependencies` derives the runtime _edge_ from the reference rather than the importer — including references that exist only as entry exports (`referenced_symbols_by_entry_point_chunk`), which are otherwise invisible to the statement walk and leave the chunk emitting a helper it never imported.
- **Node-mode provenance.** `__toESM(mod, 1)` applies when the importing module is ESM by definition format. Linking collapses the re-export chain onto the external's namespace symbol, so the reference reaching the inclusion pass may belong to a consumer several hops downstream; the importing module is recovered by walking the symbol link chain (`external_import_writer`).

The record is keyed by external namespace only, so it is **per-bundle, not per-chunk** — inclusion runs before chunking. A default import in one chunk therefore wraps the external in every chunk, including named-only ones. Named access still reads through (the wrapper installs forwarding getters), but identity and descriptor shape differ, and a Proxy-based CommonJS export sees extra traps. See the doc comment on `recorded_external_interop` for why narrowing this is not simply a chunk-membership test.
