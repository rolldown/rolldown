# Runtime Helpers

## `__commonJS` and `__commonJSMin`: release `cb` after initialization

After the first call in either helper, `mod` is set and `cb` is never accessed again. Without an explicit `cb = null`, the factory is permanently retained in the closure — a memory leak in long-lived processes (e.g. SSR servers loading bundles via `vm.createContext`).

Reference: https://github.com/rolldown/rolldown/issues/9063

## `__toESM`: deciding interop for external modules

`require("external")` returns the raw CommonJS exports, so non-ESM formats must run it through `__toESM` whenever the bundle reads it _as an ES module_ — `import * as ns`, or `ns.default`. A named-only import reads the CommonJS object directly and must not be wrapped.

The trap is that the question cannot be answered from the static imports a chunk's own modules carry. `import d from 'external'; export { d }` links the shim's symbol to a `NamespaceAlias`, so tree-shaking follows the alias straight to the external namespace and never enqueues the shim's declaring statement. With a side-effect-free external the shim is dropped entirely, while the `<external_ns>.default` reference it produced survives elsewhere. Deriving `needs_interop` from `chunk.direct_imports_from_external_modules` then answers "no" for a chunk the finalizer still renders `.default` against — a silently wrong bundle (issue #10069).

So the **inclusion pass records it** (`note_external_interop_use`), being the one place that walks the reference _after_ linking resolved it. Three consumers OR that record into their own `named_imports`-derived answer: the cjs renderer, the iife/umd renderer, and the chunk deconflicter's mixed-mode naming — all via `chunk_recorded_external_interop`.

Two things follow from the importer being potentially dead:

- **Keeping the helper alive.** `RuntimeHelper::ToEsm` is normally requested by the import statement, so it dies with the module holding it. `include_statements` re-adds it whenever any interop use was recorded, and `patch_module_dependencies` derives the runtime _edge_ from the reference rather than the importer — including references that exist only as entry exports (`referenced_symbols_by_entry_point_chunk`), which are otherwise invisible to the statement walk and leave the chunk emitting a helper it never imported.
- **Node-mode provenance.** `__toESM(mod, 1)` applies when the importing module is ESM by definition format. Linking collapses the re-export chain onto the external's namespace symbol, so the reference reaching the inclusion pass may belong to a consumer several hops downstream; the importing module is recovered by walking the symbol link chain (`external_import_writer`).

Inclusion runs before chunking, so the record cannot name a chunk directly. It stores the **observer** instead — the module whose included code holds the surviving reference — and rendering resolves that to a chunk via `ChunkGraph::module_to_chunk`. The observer is deliberately not the importer: the importer is routinely the shim that gets dropped, whereas an observer has included code by construction and therefore lands in a chunk.

`chunk_recorded_external_interop` then answers per chunk:

- **Which chunks wrap.** Only those an observer landed in. A chunk holding just a named import of the same external reads the CommonJS object directly. Wrapping it would not break value reads — `__copyProps` installs forwarding getters — but `__toESM` returns a fresh object (`__create(__getProtoOf(mod))`) and eagerly walks `__getOwnPropertyNames`/`__getOwnPropertyDescriptor`, changing namespace identity and descriptor shape, and firing extra `ownKeys` traps on a Proxy-based export.
- **Which mode.** Modes are aggregated per observer, not bundle-wide. A `.mjs` and a `.js` module importing the same external into separate chunks each observe it in one mode; unioning across the bundle would make both chunks look mixed-mode and emit `__toESM(mod, 1)` _and_ `__toESM(mod)`, running that eager walk twice for one live binding.

An observer that ends up without a live chunk cannot be attributed, so every chunk emitting the external honours it — contributing only that observer's mode, not the bundle union. This fallback is load-bearing: narrowing it away under-approximates, which is what produced #10069.

A chunk that wraps also needs `__toESM` in its depended symbols, and the two decisions have to agree. Over-supplying costs output: a module that gains the edge without wrapping puts `__toESM` in its chunk's depended symbols, and the resulting cross-chunk import loses its binding to DCE while the side-effectful `require` of the runtime chunk survives — a bare `require("./rolldown-runtime.js")` for a helper nothing calls. Under-supplying is worse: a chunk that wraps without the edge references a helper it never imported, and finalization panics looking up the missing cross-chunk binding.

They agree because the wrap arrives through exactly **two channels**, each carrying its own edge:

- **Recorded observations.** `chunk_recorded_external_interop` wraps the chunks observers landed in, and `patch_module_dependencies` narrows the edge to those same observers. Entry-export references belong here and reach neither the statement walk nor any `named_imports`, so `note_external_interop` runs over `referenced_symbols_by_entry_point_chunk` too — dropping that call panics (`external_interop_default_reexport_only_reachable_via_entry_export`).
- **Statically written imports.** `chunk_external_interop_modes` also wraps on the chunk's own `named_imports`, with no liveness filter: a default or namespace import whose binding nothing reads still wraps, and records no observer. The same `specifier_needs_interop` predicate supplies its edge twice over — `reference_needed_symbols` registers `__toESM` against the kept import statement, and `compute_chunk_imports` re-adds the cross-chunk import for any chunk whose direct external imports need interop (`external_interop_dead_default_import_keeps_helper`).

So the invariant is pairwise, not one source answering both questions. Narrowing a channel's wrap without its edge — or the more tempting inverse, registering `ToEsm` only for imports tree-shaking kept alive — reopens the panic for the cases the other channel does not cover.

Chunks do not exist yet when `patch_module_dependencies` runs, so `is_included` stands in for "will have a chunk". An observer that is not included cannot be attributed to one, and rendering falls back to wrapping every chunk emitting the external — so every referencing module keeps the edge, matching that fallback.
