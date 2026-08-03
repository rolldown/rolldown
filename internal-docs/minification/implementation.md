# Minification implementation

Rolldown minifies JavaScript after chunks have been rendered and after `renderChunk` hooks have
run. `GenerateStage::minify_chunks` parses each rendered chunk into a fresh Oxc AST, runs the Oxc
minifier, prints it, and collapses the new source map with the chunk's existing map.

## Property-name mangling

When `minify.mangleProps` is enabled for multiple rendered ECMAScript chunks, Rolldown first parses
those chunks in parallel and collects local property frequencies and reserved names. Those states
are merged and assigned once, producing one mapping for all rendered ECMAScript chunks processed by
the minifier. The existing parallel minification pass then parses each chunk again, applies that
shared mapping, and runs compression and identifier mangling before code generation. ASTs are not
retained between passes, so AST memory remains bounded by the allocator pool instead of the number
of chunks.

The extra parse and read-only collection traversal happen only when property mangling is enabled
for multiple ECMAScript chunks. A single eligible chunk uses Oxc's normal single-program property
mangling path. Multiple chunks require coordination because all chunks must contribute candidates
before any chunk can be rewritten, while the normal minification pass owns and releases each chunk
AST independently.

Rolldown also does not add generated mappings to its Rollup-compatible output result. Callers that
need stable mappings across builds with different output graphs must provide the relevant cache
entries themselves.

Property mangling is syntax-based and unsafe for indirect access. Rolldown rewrites recognized
property-name syntax and explicitly annotated key strings; it cannot infer that an arbitrary string
or runtime value contains a property name. The same limitation applies when an earlier transform
lowers a property into an unannotated helper-call string, for example for lower-target class fields,
object-rest exclusion lists, or decorator metadata. Module export names are also not property names,
so properties accessed through imported namespace objects must be reserved. Exclude or reserve
names used across any of these boundaries.

The shared mapping covers rendered ECMAScript chunks processed by the minifier, including code
returned by `renderChunk`. It does not cover external code, emitted prebuilt chunks,
`postBanner`/`postFooter`, chunks whose output filenames end in `.d.ts`, `.d.cts`, or `.d.mts`, or
code added or changed later by `generateBundle`. Reserve properties shared across those boundaries,
or coordinate an explicit cache when both sides are under the caller's control.
