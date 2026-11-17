# Module Sourcemap Ownership

## Summary

Rendered module code is shared with `RenderedModule` for lazy plugin/output metadata access.
Its sourcemap belongs exclusively to chunk assembly, so joining can move map strings and
discard per-module token buffers without retaining them in the final module metadata.

## Rendering and joining

`render_ecma_module` builds mapless source pieces: an optional debug marker followed by the
code body. `RenderedModuleContent` holds their shared `Arc` and, only when a map exists,
a boxed `SourceMapSource<ModuleBody>`. `ModuleBody` shares the same pieces and exposes the
last piece's code. Its newline count is precomputed during module rendering.

`RenderedModuleSource` stores only `ModuleIdx` and optional content. Module id and execution
order are read from the module table when constructing `RenderedModule` metadata.

All format renderers, including ESM inline common chunk factories, use `append_to`. It
appends the marker before transferring the existing mapped-source box through
`SourceJoiner::append_source_dyn`. Without a map it borrows the shared source pieces as before.

The existing joiner calls `take_sourcemap` and `add_sourcemap_owned`: strings move into the
chunk map, while tokens are copied/rebased and the module map's buffers are discarded.
`RenderedModule` keeps its existing mapless source representation and code/length accessors.

## Invariants and tradeoffs

- A nonempty rendered module has its code body last; the marker has no mappings.
- Content is appended once per physical chunk. An inline record rendered into multiple
  carriers gets fresh content for each carrier, as before.
- No sourcemap is reachable through `RenderedModule`'s shared source pieces.
- Only mapped modules allocate a mapped-source box. That box transfers directly into the
  joiner without reboxing; no lock or shared mutable map is needed.
- The optional map is behind the existing source box, keeping the temporary module record
  compact even when sourcemaps are disabled. A 64-bit layout test guards its 32-byte size.

## Validation

Unit tests cover moved string allocations, shared code lifetime, null source slots, line
offsets across module boundaries, and the unmapped path. Existing integration fixtures cover
format wrappers, addons, transform chains, and inline common chunk sourcemaps.

## Related

- `../inline-common-chunks/implementation.md`
- `../bundler-data-lifecycle/implementation.md`
- `crates/rolldown/src/utils/render_ecma_module.rs`
- `crates/rolldown_sourcemap/src/source.rs`
- `crates/rolldown_sourcemap/src/source_joiner.rs`
