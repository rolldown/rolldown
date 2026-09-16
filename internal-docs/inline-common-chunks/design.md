# Experimental inline common chunks: design

`output.experimentalInlineCommonChunks` removes eligible small automatic common-chunk files. It
copies each removed chunk's scope-hoisted body into the chunks that need it as a factory, while a
shared runtime record makes all copies resolve to one logical module instance. The detailed,
standalone implementation blueprint is [implementation.md](./implementation.md).

The option is intentionally separate from `codeSplitting`. Code splitting still decides which
automatic chunks exist; `experimentalInlineCommonChunks` is a later output policy that can replace
some of those chunks. An omitted option, an omitted `maxSize`, or `maxSize: 0` selects nothing.

## Model

The unit of duplication is a chunk, not an individual module. This follows the direction recorded in
[backlog #7](https://github.com/rolldown/backlog/issues/7): modules inside the chunk remain statically
linked and scope-hoisted inside one factory, and only references that cross the inlined chunk boundary
become runtime property reads.

```text
before                                after

entry-a ─┐                        entry-a [factory S] ─┐
         ├─> small common S                              ├─> one registry record S
entry-b ─┘                        entry-b [factory S] ─┘
```

Registering a factory does not run the module body. The first `__rd_share_require(key)` creates and
caches the exports record, runs the factory, and returns that record. Later calls return the same
record. Export getters keep reassigned ESM bindings live, and consumers call function-valued exports
as plain functions rather than methods.

## Decisions

1. **Wrap-all strict execution order is part of the contract.** Backlog
   [#7](https://github.com/rolldown/backlog/issues/7) and
   [#14](https://github.com/rolldown/backlog/issues/14) require strict execution order. The option
   enables it when omitted and rejects an explicit `false`. On-demand wrapping is rejected because
   the current proof assumes every selected module body is deferred behind its init wrapper.
2. **The runtime registry is one standalone ESM chunk.** Every carrier imports it. Folding the
   registry into a user chunk could create a new import cycle, and copying it would split logical
   identity. Runtime merging and sweeping therefore stay disabled while the option is active.
3. **Factory placement uses only proven ESM ordering.** A chunk can omit a factory only when a
   static dependency in a strictly lower strongly connected component already carries it. No
   coverage is inherited within a cycle, because the first evaluated member depends on the loader's
   entry point. This resolves the registration-before-require question raised in
   [backlog #17](https://github.com/rolldown/backlog/issues/17) conservatively.
4. **Registry keys follow chunk identity, not rendered content or checkout location.** The key
   hashes the sorted, length-prefixed cwd-relative, slash-normalized stable module IDs in the
   logical chunk. Content-only edits and moving the project root keep the key stable, while
   byte-identical chunks with different modules remain distinct. This applies the criteria in
   [backlog #12](https://github.com/rolldown/backlog/issues/12).
5. **The logical chunk remains the symbol owner.** Moving a module to several owners would violate
   current `module_to_chunk` assumptions. An inlined chunk remains live for linking, naming, and
   finalization, but emits no asset. Physical carrier placements are recorded separately.
6. **Plugin chunk metadata reports physical copies.** Each carrier's `moduleIds` and `modules`
   include the modules whose factory it contains, including in filename callbacks, and the registry
   chunk reports its generated exports. This gives plugins truthful placement data, even though
   plugins that assume one module belongs to one output chunk may still need adaptation. The
   omitted logical chunk itself never invokes a filename callback or reserves an output name. The
   ecosystem risk is tracked in [backlog #9](https://github.com/rolldown/backlog/issues/9).

## Conservative boundary

The first implementation supports ES output, code splitting, `preserveModules: false`,
`preserveEntrySignatures: false`, and wrap-all strict execution order. It considers only automatic
`ChunkKind::Common` chunks below the configured source-byte threshold.

It leaves a chunk as a normal file when the factory model cannot represent it with the current
pipeline: the runtime chunk, manual chunks, emitted chunks, dynamic-import targets, chunks containing
top-level await, chunks containing a dynamic import, chunks containing `import.meta`, chunks with
external dependencies, chunks with a static dependency that is neither the registry nor another
selected candidate, chunks containing direct `eval`, chunks with unresolved references to generated
factory protocol names, candidates directly consumed by a chunk containing direct `eval`, and
live-binding owners re-exported by a different chunk.

The re-export exclusion applies when the final chunk export table requires a native cross-chunk
re-export. Scope-hoisted source re-exports that canonicalize directly to the owning chunk can still
cross the runtime boundary through its exports-object getter.

Dynamic imports are excluded because one factory body is rendered once but can be copied into
carriers in different directories. `import.meta` is excluded because it identifies the physical
output module, which differs between copies. External and non-selected static dependencies are
excluded because moving their import declarations can change relative resolution or sibling
side-effect order. Top-level await is excluded because the registry API is synchronous, matching
the initial contract in the
[RFC](https://github.com/rolldown/rolldown/discussions/10693).

Direct `eval` is excluded at both sides of a moved boundary because opaque strings can observe new
lexical bindings. Ordinary unresolved globals are supported: carrier deconfliction reserves every
copied body's unresolved names, and registry imports use consumer-local conflictless aliases.

## Costs

- Removing requests increases total emitted JavaScript because factories have several physical
  placements.
- Selection computes selected-chunk reachability and static chunk strongly connected components.
- Inlined chunks are named before carriers, which makes deconfliction partly sequential; carriers
  conservatively reserve the canonical names used by copied bodies.
- A source map for the logical body is cloned into each carrier so every physical copy remains
  mapped.
- A positive option keeps the runtime chunk standalone even if all candidates are later excluded;
  the current pipeline decides runtime placement before inline selection.

These costs are accepted for the experimental, correctness-first implementation. A later chunking
refactor should preserve the observable behavior and make logical ownership, physical placements,
and registry needs first-class data instead of late annotations on `Chunk`.

## Related documents

- [Implementation blueprint](./implementation.md)
- [Current code-splitting design](../code-splitting/design.md)
- [Current code-splitting implementation](../code-splitting/implementation.md)
