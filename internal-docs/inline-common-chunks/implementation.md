# Inline Common Chunks — Implementation

The rationale is in [design.md](./design.md). This file describes where the feature lives and what
each part guarantees.

## Option

`output.codeSplitting.inlineCommonChunks.maxSize`, in pre-render module bytes. It reaches the core as
`ManualCodeSplittingOptions::inline_common_chunks`; `NormalizedBundlerOptions::inline_common_chunks_max_size`
and `is_inline_common_chunks_enabled` are the only readers. Zero is off.

## Pipeline position

```
compute_cross_chunk_links()          final static chunk graph + symbol->chunk table
   │
   ├─ plan_inline_common_chunks()    selection, placement, rewiring   (inline_common_chunks.rs)
   │
   ├─ deconflict_chunk_symbols()     inlined chunks first, then carriers with reservations
   │
   ├─ finalize_modules()             consumer references become `<binding>.<export>`
   │
   └─ render_chunk_to_assets()       phase A renders factories, phase B renders files
```

The pass must run after `compute_cross_chunk_links` because it needs `cross_chunk_imports` and
`SymbolRefDataClassic::chunk_idx`, and before deconfliction because a carrier has to reserve the
names an inlined chunk's body already uses.

## Selection (`plan_inline_common_chunks`)

A candidate is a live `ChunkKind::Common` chunk whose `ChunkReasonType` is `Common` — automatic
code splitting only, never a manual group chunk or its `maxSize` split. It is rejected when it is the
runtime chunk, an emitted chunk, a dynamic import target, contains top-level await, contains a
dynamic import or a resolved file URL (see design.md), exceeds `maxSize` measured with
`Module::size()`, or has no importer.

`ROLLDOWN_INLINE_COMMON_CHUNKS_LEDGER=<path>` writes the selection and placement ledger as JSON. It
is a research hook, not a supported output.

## Placement

`reach(X)` is every selected chunk `X` pulls in through chains of selected chunks; a carrier of a
factory must also carry everything that factory executes.

Redundant-placement elimination runs over the condensation of the static chunk graph
(`strongly_connected_components`, iterative Tarjan). Components come out dependencies-first, and a
chunk inherits coverage only from dependencies in strictly lower components — see design principle 2.
`available[X]` is what is guaranteed registered before `X` runs; `carried[X]` is `reach(X)` minus
that.

A selected chunk that ends with no carrier is de-selected and stays a file. That cannot happen with
the current rules, and the check exists because the failure mode is silent code loss rather than a
build error.

## Rewiring

After placement no chunk imports an inlined chunk as a file:

- a carrier takes over the inlined chunk's own cross-chunk and external imports;
- every carrier and consumer gains an import of the registry chunk;
- `imports_from_other_chunks` entries pointing at inlined chunks are dropped, on both sides.

`required_inline_chunks` records what each chunk must execute with `__rd_share_require`; for an
inlined chunk that list is executed at the top of its own factory.

## Naming

`deconflict_chunk_symbols` gains two inputs: a reserved-name list and a flag. While the feature is
on it reserves `__rd_share`, `__rd_share_require`, and the three factory parameter names in every
chunk, so a user symbol of the same name is renamed instead. Inlined chunks are deconflicted first,
in sequence, each reserving the names of the peers it shares a carrier with; every carrier then
reserves the full name set of each chunk it carries. `inline_binding_names_for_other_chunks` holds
the local name for each required chunk's exports object.

## Emission (`ecmascript/format/share_factory.rs`)

Phase A of `instantiate_chunks` renders every inlined chunk into one `__rd_share(id, factory)`
registration and keeps its rendered-module map. Phase B renders the remaining chunks; an inlined
chunk produces no asset.

A carrier prints, after its own imports: the inlined chunks' import declarations re-rendered against
its own directory, then their factories, then `var <binding> = __rd_share_require(<id>);` for
everything it must execute. The registry chunk additionally prints the registry itself.

`render_factory_exports` publishes the chunk's `exports_to_other_chunks` as getters through the
factory's third parameter, which is why a reassigned export stays live across the boundary.

## Consumer references

`finalized_expr_for_inlined_chunk_symbol` in the module finalizer turns a reference whose canonical
symbol lives in an inlined chunk into `<binding>.<export>`. Two reachability gates had to learn about
it, because a wrapper that is neither declared nor imported by a chunk used to be unreachable there:
`wrapper_is_reachable_in_chunk` and `collect_entry_reexported_wrapper_inits`, which now take a
predicate instead of the chunk's name map.

## Ledger

`generate_rendered_chunk` merges each carried chunk's rendered modules into the carrier's
`chunk.modules` and its `module_ids`, so a duplicated module appears in every chunk that carries its
factory.

## Runtime

`render_registry` prints the registry into the runtime chunk and exports `__rd_share` and
`__rd_share_require`. `__rd_share_require` caches the module record before running the factory, so a
cycle re-entering it sees the partially populated exports object; a factory that throws records the
error and rethrows it on every later require.

## Constraints this feature places on other passes

- `try_merge_runtime_chunk` returns early while the feature is on, so the registry stays in a
  standalone runtime chunk that cannot close an import cycle with a carrier.
- `sweep_unused_runtime_module` returns early for the same reason: the registry needs that chunk even
  with zero helper demand.

## Related

- [design.md](./design.md)
- `../code-splitting/implementation.md`
