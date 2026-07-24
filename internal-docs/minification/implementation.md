# Minification implementation

Rolldown minifies JavaScript after chunks have been rendered and after `renderChunk` hooks have
run. `GenerateStage::minify_chunks` parses each rendered chunk into a fresh Oxc AST, runs the Oxc
minifier, prints it, and collapses the new source map with the chunk's existing map.

## Property-name mangling

`minify.mangleProps` is passed to Oxc during the existing per-chunk minification pass. Each
rendered ECMAScript chunk is parsed once, then property mangling, compression, and identifier
mangling run on that AST before code generation.

Generated property mappings are local to a chunk. This is a known limitation: a property used by
separate output chunks can receive different names because each chunk has a different frequency
table. If an unpinned property receives inconsistent names, Rolldown emits a
`CROSS_CHUNK_PROPERTY_MANGLE` warning. The warning is detected from the caches returned by Oxc in
linear time after the parallel pass; it does not require another parse. It does not make the
output safe: callers must provide an explicit `cache` mapping for every matched property that
crosses a chunk boundary.

Global mapping collection would require restructuring the current independent per-chunk pass so
all property candidates are coordinated before any chunk is rewritten (or adding another parse).
Support can be added if real-world demand justifies that cost and complexity.

Rolldown also does not add generated mappings to its Rollup-compatible output result. Callers that
need stable mappings across builds with different output graphs must provide the relevant cache
entries themselves.

Property mangling intentionally operates on the final JavaScript syntax. Transformer spans and
property-key provenance belong to earlier module ASTs and must not cross the print-and-reparse
boundary. Options such as quoted-property mangling therefore apply to the JavaScript emitted by
TypeScript and other transforms, as well as code added by render hooks.
