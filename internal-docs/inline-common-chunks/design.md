# Inline Common Chunks — Design & Principles

## Summary

`output.codeSplitting.inlineCommonChunks` replaces small automatic common chunks with factory
definitions placed in the chunks that consumed them, linked at runtime through one shared registry.
The chunk stops being a request while its modules keep one logical instance per JavaScript realm.
The option defaults to `maxSize: 0`, which selects nothing and leaves output byte-identical to a
build without the option.

This is a research prototype for the RFC "Progressive Module Runtime and
`codeSplitting.inlineCommonChunks`". It is not an API to keep. [implementation.md](./implementation.md)
describes the machinery.

## Why the unit is a chunk, not a module

The RFC's guide-level examples wrap one module per `__share` call and then describe merging the
blocks of one chunk back into a single closure as an optimization ("Optimal output"). This
implementation only ever emits the merged form: one `__share(id, factory)` per inlined chunk, holding
that chunk's whole scope-hoisted body.

Two consequences follow. Modules inside an inlined chunk keep static linking to each other, so the
minifier can still rename and inline across them; only the chunk boundary becomes a runtime property
lookup. And the registry key is a chunk, so ids are small integers assigned in evaluation order
rather than module paths.

## Principles

1. **One registry per realm.** The registry is printed into the chunk that holds the runtime module,
   which every other chunk already imports. A second copy would fork the factory and module tables
   and destroy logical module identity, so the runtime chunk is never a selection candidate, and
   while the feature is on it is never folded into a user chunk and never swept away.
2. **Placement never invents an ordering it cannot prove.** A chunk may skip carrying a factory only
   when a chunk that carries it is guaranteed to have finished evaluating first. Under ESM that is
   exactly "a dependency in a strictly lower strongly connected component". Inside one component the
   evaluation order depends on which root the loader entered from, so nothing is inherited there.
3. **Selection is conservative and stated.** Anything the emission model cannot represent is left as
   a file rather than handled approximately: top-level await, dynamically imported chunks, and chunks
   whose body would contain a chunk-relative path (see below).
4. **The chunk ledger stays true.** A carried module is reported in the carrier's `chunk.modules`, so
   `generateBundle` consumers see the duplication. An implementation that injected factories during
   rendering without updating the ledger would make every downstream placement measurement wrong.

## Why a factory body cannot contain a chunk-relative path

An inlined chunk is rendered once and its text is printed into every carrier. Carriers can sit in
different output directories, so any path the body computes relative to its own file would be wrong
in at least one of them. Two constructs do that: a dynamic import specifier and a resolved
`import.meta.ROLLDOWN_FILE_URL_*` reference. Selection rejects chunks containing either.

Rendering the body per carrier would not help. Module ASTs are finalized once, against the chunk that
owns the module, so those paths are already baked in before any carrier exists.

## Why references become property reads

A consumer of an inlined chunk can no longer import a binding from a file. It reads
`<binding>.<export>` on that chunk's exports object instead, and the factory publishes its exports as
getters. That is what keeps a reassigned export live across the boundary; a destructuring binding
would snapshot it.

The mechanism is the one Rolldown already uses for CommonJS cross-chunk references
(`finalized_expr_for_cross_chunk_symbol`), applied to ESM output for inlined chunks only.

## Rejected alternatives

- **Moving modules between chunks (`module_to_chunk`).** `chunk_graph.rs` documents that a module
  maps to one live chunk and that `add_depended_symbol_with_wrapped_esm_init` relies on it. An
  inlined chunk therefore stays live and keeps owning its modules; only its emission changes. The
  carrier's extra modules live in a separate `carried_inline_chunks` list, so no pass that assumes
  ownership sees them.
- **A registry on `globalThis`.** It would remove the dependency on a runtime chunk, but it makes the
  registry visible outside the bundle and costs a lazy-initialization guard in every carrier.
- **Destructuring the exports object in the consumer.** Smaller output, but it loses live bindings,
  which is one of the semantics the RFC promises to preserve.

## Unresolved questions

- Placement is per consumer with elimination across component boundaries. The RFC's "minimum
  placement" optimization — pushing a factory down into a common chunk shared by several consumers —
  is not implemented.
- Deconfliction reserves a carried chunk's whole name set in its carriers, and co-hosted inlined
  chunks are named in sequence. Both are safe over-approximations that cost some name quality.
- Source maps for a carried factory body are dropped; the body is appended as a plain source.
- Only ESM output is supported. Other formats resolve cross-chunk references through their own
  binding shapes.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this
- `../code-splitting/implementation.md` — where chunks come from, and the runtime chunk placement
  rules this feature constrains
