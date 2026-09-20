# Inline Common Chunks — Design & Principles

## Summary

`output.codeSplitting.experimentalInlineCommonChunks` lets a small common chunk stop being a file. Its modules are printed, in full, into every file that reads them, inside a registry factory (`__share(id, (exports) => { ... })`), and each reading file obtains the shared exports object with `__share_require(id)`. The registry lives in the runtime module, so however many copies of the factory a page loads, the modules run once and every reader sees the same live bindings. The option trades duplicated bytes for one fewer request per removed chunk. It is experimental and only exists for ESM output under `strictExecutionOrder`.

The contract the feature keeps, and that every test in `crates/rolldown/tests/rolldown/function/inline_common_chunks/` and `packages/rolldown/tests/behaviors/inline-common-chunks/` checks against the same build with the option off:

- Every module still evaluates exactly once, in the order strict execution order gives it, whichever entry or dynamic entry loads first.
- Consumers read live bindings; CommonJS members keep one `module.exports` object; `init_*`/`require_*` wrappers are shared.
- A call through a bridge sees `this === undefined` for `f()`, `f?.()`, ` tag` `` and namespace-member calls, exactly as a call through an import does today.
- Errors thrown while a shared module executes are rethrown identically to later readers.
- `maxSize: 0` (the default) or an absent option produces byte-identical output to a build without the feature.

The machinery is in [implementation.md](./implementation.md).

## Design principles / Goals

1. **The unit is a whole common chunk.** A selected chunk (a "record") keeps its module set, its wrappers and its `exports_to_other_chunks`; nothing is inlined symbol by symbol. This keeps every question ("who owns this symbol", "what does this chunk export") answerable by the existing passes.
2. **A record stays a live chunk.** `ChunkGraph::module_to_chunk`, `chunk.modules`, exec order, namespace decisions and cross-chunk linking never see a difference. The only new state is `InlineCommonChunksState` (selected records, who reads them, who carries them, bridge names), owned by the generate stage and read by naming, deconflicting, the finalizer and the renderers. Records use no `PostChunkOptimizationOperation` variant: `Removed` means "merged elsewhere" to its readers, and a record is neither merged nor rendered.
3. **Decide after lowering, project after linking.** Selection runs in `finalize_chunk_plan` after `apply_order_wraps` (so "every member has a wrapper" is a fact) and before the unused-runtime sweep (so the registry demand it registers keeps the runtime). It reads a provisional, read-only cross-chunk link pass to see the edges. The final `compute_cross_chunk_links` then derives every edge from scratch, and only afterwards are the logical edges projected onto files (carriers take over a record's imports, imports of records disappear). No pre-selection conclusion about edges is reused.
4. **Every reader carries the full factory; the closure is carried too.** A file that reads record R prints R's factory, and also the factory of every record R reads, transitively. There is no minimal-placement search and no deduplication across files: a page that loads two carriers ships two copies, and the registry ignores the second. Correctness never depends on which file loads first.
5. **Registration comes first, first registration wins, and every `__share_require(id)` is preceded by `__share(id, ...)` in the same file.** Registrations and bridges are printed right after a file's imports, before any of the file's own code; a record's own bridges are printed at the top of its factory, which only runs when required. The runtime chunk is imported first.
6. **The runtime stays standalone while the feature is on.** The registry demand appears after every runtime-merge proof has run, so `try_merge_runtime_chunk` returns immediately when the option is enabled. A build with nothing selected therefore differs from the off build by one standalone `rolldown-runtime.js`; that difference is in scope.
7. **When unsure, keep the file.** Every rule in the selection list below rejects rather than special-cases. A rejected candidate is rendered exactly as before, so a rule can be relaxed later without changing output for the cases it accepts today.
8. **No new pipeline machinery.** Runtime demand is registered through the synthetic-statement channel `OrderWrapState` already uses for order wrappers, so the sweep gate, the symbol-to-chunk table, deconfliction, the finalizer's runtime filter and the runtime symbol closure all see it without new branches.

## Why each candidate rule exists

| A chunk stays a file when …                                                                                       | Because …                                                                                                                                                                                                                         |
| ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| it is not `ChunkKind::Common` with `ChunkReasonType::Common`, hosts an entry module, or is emitted                | Entries and manual groups are user-visible files with their own contracts; the implementation chunk a strict entry facade leaves behind hosts the entry's body and trigger.                                                       |
| some `import()` resolves to it, or it hosts a collapsed dynamic-entry facade                                      | A dynamic import needs a file to fetch.                                                                                                                                                                                           |
| its pre-render size (sum of `Module::size()`) is not strictly below `maxSize`                                     | The option's only knob; measured before rendering so the decision does not depend on output settings.                                                                                                                             |
| a member has top-level await                                                                                      | A factory is synchronous; the wrappers would have to be awaited from inside `__share_require`.                                                                                                                                    |
| retained code uses `import.meta`, `import.meta.ROLLDOWN_FILE_URL_*`, a live dynamic import, or an external import | These are resolved relative to the file the code is printed in, and a factory is printed into several files; rewriting them to a carrier's location would silently change asset resolution.                                       |
| a member, or a module referencing the chunk's symbols, uses direct `eval`                                         | `eval` reads bindings by name; a bridge turns `x` into `bridge.x`.                                                                                                                                                                |
| a member matches `exclude` or is a `.d.ts`/`.d.mts`/`.d.cts` file                                                 | User opt-out, with the same matcher rules as `codeSplitting.groups[].test`.                                                                                                                                                       |
| a member has no wrapper (`init_*`/`require_*`) or is a concatenated wrapped module                                | The factory may only define; it must not run user code. Under strict execution order every member is wrapped, so this rule is a guard, not a filter.                                                                              |
| another chunk re-exports one of its symbols with `export { x }`                                                   | An ESM export needs a local binding; a bridge property read cannot be forwarded as a live binding.                                                                                                                                |
| no file imports it                                                                                                | A record without a reader would simply vanish.                                                                                                                                                                                    |
| it is in a static-import cycle with a file                                                                        | A carrier prints a record's imports as its own; a file that both reads the record and is imported by it would have to import itself. Cycles made only of records are fine: records reach each other through bridges, not imports. |

## Rejected alternatives

- **Per-symbol inlining or code motion.** Would need new ownership rules for symbols and a second finalizer path. The whole-chunk unit reuses everything.
- **Forwarding record exports with ESM `export { x }` in carriers.** Not expressible for a property read; hence the re-export rule above instead of a new emission mode.
- **Projecting edges inside `compute_cross_chunk_links`.** That pass is a pure derivation with two read-only callers that need logical edges (`predicted_static_import_edges`, `lowered_static_import_edges`). Projecting once, right after it, keeps one writer and leaves the derivation untouched.
- **A `PostChunkOptimizationOperation` variant for records.** Its readers implement "this chunk was merged into another" and "do not render"; a record is live and not rendered, which would have turned two-state checks into three-state ones across the stage.
- **A `globalThis` registry.** The registry is a runtime-module binding, so it is scoped to one bundle and participates in tree shaking, naming and hashing like every other helper.
- **Renaming a record per carrier.** A factory is rendered once and its text is shared; instead, records are deconflicted first and carriers reserve the record's module-scope names (see implementation).
- **A minimal or deduplicated placement.** Would make correctness depend on load order between files. Rule 4 makes every file self-sufficient.

## Unresolved questions

- Registry ids are chunk names made unique within one build; nothing promises stability across builds, so two bundles built separately must not share a page's registry.
- Cost model: `maxSize` bounds the duplicated source, not the duplicated output; a record printed into many files multiplies its bytes. A count limit or a shared-carrier heuristic is future work.
- CSS, dev mode/HMR, `preserveModules`, `onDemandWrapping` and non-ESM formats are out of scope and rejected at option validation.
- Vite's manifest, modulepreload and SSR integrations have not been adapted; a record has no file, so anything keyed on chunk files sees only carriers.

## Related

- [implementation.md](./implementation.md) — the machinery
- `../code-splitting/implementation.md` — chunk creation, runtime placement, the unused-runtime sweep this hooks into
- `../code-splitting/design.md` — strict execution order and `OrderWrapState`
- `../runtime-helpers/implementation.md` — the registry helpers
