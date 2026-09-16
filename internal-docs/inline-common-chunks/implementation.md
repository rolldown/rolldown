# Inline Common Chunks — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`output.codeSplitting.experimentalInlineCommonChunks` adds one struct, `InlineCommonChunksState` (`crates/rolldown/src/stages/generate_stage/inline_common_chunks/`), and threads it through the generate stage. Selection and placement happen in `finalize_chunk_plan`; the physical file graph is derived right after `compute_cross_chunk_links`; naming, deconfliction, the module finalizer and the ESM renderer read the state. Three runtime helpers (`__share`, `__share_require`, `__share_export`) implement the registry.

## Options

| Layer      | Where                                                                                                                                       | What                                                                                                                                                                                                                                                                                                        |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TypeScript | `packages/rolldown/src/options/output-options.ts` (`ExperimentalInlineCommonChunksOptions`), `validator.ts`, `bindingify-output-options.ts` | `{ maxSize?: number, exclude?: string \| RegExp \| fn \| Array }`. `exclude` functions are batched into one napi call like `groups[].test`.                                                                                                                                                                 |
| Binding    | `crates/rolldown_binding/src/options/binding_output_options/binding_manual_code_splitting_options.rs`                                       | `BindingInlineCommonChunksOptions`                                                                                                                                                                                                                                                                          |
| Common     | `crates/rolldown_common/src/inner_bundler_options/types/inline_common_chunks_options.rs`                                                    | `InlineCommonChunksOptions` (raw, deserializable for fixtures) and `NormalizedInlineCommonChunksOptions { max_size: u64, exclude }`; `NormalizedBundlerOptions::is_inline_common_chunks_enabled()` is `max_size > 0`.                                                                                       |
| Validation | `crates/rolldown/src/utils/prepare_build_context.rs` (`verify_inline_common_chunks_options`)                                                | `maxSize` must be a non-negative safe integer. `maxSize > 0` requires `format: 'es'`, `strictExecutionOrder: true`, an explicit `preserveEntrySignatures: false`, no `preserveModules`, no `experimental.devMode`, no `experimental.onDemandWrapping`. Errors are `InvalidOptionType::InlineCommonChunks*`. |

The test config variant field `inlineCommonChunksMaxSize` (`crates/rolldown_testing_config/src/config_variant.rs`) lets one fixture run with the option on and off.

## Pipeline position

```
generate()
  ├─ prepare_inline_common_chunks()          evaluates `exclude` once (async), marks `.d.ts`
  ├─ generate_chunks()                        try_merge_runtime_chunk() returns early while on
  ├─ finalize_chunk_plan()
  │    ├─ apply_order_wraps()                 fold skipped (it goes through try_merge_runtime_chunk)
  │    ├─ select_inline_common_chunks()       <- selection point
  │    └─ sweep_unused_runtime_module()       gated on OrderWrapState demand, which now includes the registry
  ├─ compute_cross_chunk_links()              records are live chunks; their edges are derived like any other
  ├─ apply_inline_common_chunks_links()       <- physical projection
  ├─ generate_chunk_name_and_preliminary_filenames()   record id; carriers' moduleIds
  ├─ deconflict_inline_records() + deconflict_chunk_symbols()
  ├─ finalize_modules()                       bridge rewrites
  └─ render_chunk_to_assets()                 render_inline_records() (phase A), then files
```

## State

```rust
pub struct InlineCommonChunksState {
  enabled: bool,
  excluded_modules: FxHashSet<ModuleIdx>,
  records: FxIndexMap<ChunkIdx, InlineRecord>,           // selected chunks, exec order
  readers: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,           // file or record -> records read directly
  carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,           // file -> records whose factories it prints, deps first
  bridge_names: FxHashMap<ChunkIdx, FxHashMap<ChunkIdx, CompactStr>>, // reader -> record -> local binding
  runtime_chunk: Option<ChunkIdx>,
}
pub struct InlineRecord { pub id: ArcStr, pub exports_param: CompactStr }
```

`ChunkGraph` and `Chunk` gain no fields. A record's `preliminary_filename` and `pre_rendered_chunk` stay `None`; `resolve_file_urls`, `detect_ineffective_dynamic_imports` and `instantiate_chunks` skip records.

## Selection (`inline_common_chunks/select.rs`)

Runs in `finalize_chunk_plan` after order lowering and before the unused-runtime sweep. It computes `compute_wrapped_esm_init_metadata` and a read-only `compute_cross_chunk_link_state` on the lowered graph to see static importers, exported symbols, dynamic-import targets and external imports, then applies the rule list from `design.md` to every live chunk in `sorted_chunk_idx_vec` order (`keep_as_file_reason`). Two rules need whole-graph facts: chunks owning a symbol another chunk exports (`foreign_exported_owner_chunks`) and chunks whose symbols a direct-`eval` module references (`eval_read_chunks`). `import.meta` is found by walking the included statements of a member with `oxc::ast_visit::VisitJs` (`program.body[i]` is `stmt_infos[i + 1]`).

After the per-chunk rules, a Tarjan SCC over the static chunk graph (runtime chunk excluded) drops every candidate that shares a component with a non-candidate. Then placement runs, and a candidate no file ends up carrying is dropped and placement rerun.

Every decision is logged: `RD_LOG=rolldown::inline_common_chunks=debug` prints `kept as file` with the rule, `selected as record` with size and readers, and each carrier's takeover.

## Placement (`inline_common_chunks/place.rs`)

`readers[c]` = the records among `c`'s static importees, in record exec order (computed for files and records). `carried[f]` for a file `f` = depth-first post-order over `readers` starting from `readers[f]`, so a record's dependencies come first and each record appears once. A record read only by other records that no file reads is an orphan and is dropped.

## Runtime demand

For every carrier: `OrderWrapState::insert_runtime_helper_demand(carrier, Share | ShareRequire)`. For every record: `ShareExport`, plus `ShareRequire` when it reads another record. Then `compute_runtime_symbol_closure` pulls `__share_factories`, `__share_records`, `__create` and `__defProp` in. This is the same synthetic-statement channel order wrappers use, so `required_runtime_helpers()` (sweep gate, finalizer runtime filter), `collect_depended_symbols` (import from the runtime chunk), `forces_runtime_stmt` and deconfliction all see it. If the runtime module has no standalone chunk at this point (`ensure_runtime_module_for_order_wraps` is reused), one is created before the demand is registered. Under the option's preconditions every module is wrapped, so the runtime already has a chunk; the branch is defensive.

## Physical projection (`inline_common_chunks/rewire.rs`)

Right after `compute_cross_chunk_links` wrote `imports_from_other_chunks` and `cross_chunk_imports`:

- `readers`/`carried` are recomputed from the final edges (the record-to-record part must match the selection-time result; `debug_assert`).
- A record drops its imports of other records and keeps the rest (runtime helpers, non-record files).
- A carrier drops its imports of records, adds every carried record's `cross_chunk_imports`, and imports the runtime chunk first (`imports_from_other_chunks` key moved to index 0, `cross_chunk_imports` sorted with the runtime first).

`exports_to_other_chunks` is untouched: a record's export names become the getter keys of its exports object, and consumers resolve them the same way they resolve cross-chunk names today.

## Naming

In `generate_chunk_name_and_preliminary_filenames` a record gets the `[name]` any common chunk would get, made unique with a numeric suffix within the build; that is its registry id (`InlineRecord::id`). No filename template runs for it. A carrier's `pre_rendered_chunk.module_ids` lists its own modules, then the carried records' modules, so `chunkFileNames` and `RenderedChunk.moduleIds` see everything the file contains.

## Deconflicting

A record's factory contributes exactly its import bindings to the module scope of a carrier; everything else it declares lives inside the factory function. So:

1. `deconflict_inline_records` (`inline_common_chunks/deconflict.rs`) names records first, one after another. A record reserves the import-binding names of every already-named record it shares a carrier with, and the unresolved global references of every chunk it shares a file with (so no carrier or co-carried factory can shadow a global the record reads). When it imports a symbol another record already named, it takes the same name (`Renamer::preassign_symbol`), so the carrier prints one import specifier for both. It also names its factory parameter (`exports`, moved aside if the record's code reads a global `exports`) and, for each record it reads, a factory-local bridge binding.
2. Every carrier then reserves the import-binding names and global references of the records it carries, takes the same name for symbols it imports itself, and names one bridge binding per record it reads (`share_<id>`, avoiding nested-scope bindings in its modules because the bridge is referenced wherever a record symbol was). `deconflict_chunk_symbols` takes this as `InlineDeconflictPlan` and returns the bridge names.

The ESM renderer merges the carrier's and the carried records' import specifiers per importee and drops exact duplicates.

## Consumer references (module finalizer)

`ScopeHoistingFinalizer::inline_bridge_member(canonical_ref)` answers `Some((bridge, export_name))` when the symbol's chunk (`symbol_db.get(ref).chunk_idx`, committed by the final link pass) is a record the current chunk reads. Hooks:

- `finalized_expr_for_symbol_ref` renders `bridge.export_name` and, when the expression is a callee, `(0, bridge.export_name)`, the same guard namespace-member callees use.
- `visit_expression` adds the guard for the forms the callee flag did not cover: a member callee that resolves to exactly a bridge member (`ns.f()`), the callee of an optional call inside a `ChainExpression` (`f?.()`, `ns.f?.()`), and a tagged-template tag.
- `wrapper_is_reachable_in_chunk` counts a bridged wrapper as reachable, so `init_*()` calls into a record are emitted as `bridge.init_x()`.
- `GenerateContext::finalized_string_pattern_for_symbol_ref` (entry prologues, `render_wrapped_entry_chunk`) takes the same path.

A record's own modules are finalized once, with the record chunk as their context.

## Rendering

`render_chunk_to_assets` renders every record first (`render_record`, `crates/rolldown/src/ecmascript/format/share_factory.rs`) with a `GenerateContext` for the record chunk: the module sources (with source maps, `initial_indent = 1`), a prelude `(exports) => {` plus `var <bridge> = __share_require("<id>")` for each record it reads, and an epilogue `__share_export(exports, { name: () => local, ... }); }` built from `get_export_items`. The `RenderedRecord`s live for the duration of file instantiation and are borrowed by every carrier through `GenerateContext::inline_renders`.

`render_esm` prints, for a carrier, after the merged imports and before anything else:

```js
import { i as __share_require, n as __share, r as __share_export, t as __esmMin } from "./rolldown-runtime.js";
__share("shared", (exports) => {
	//#region shared.js
	var count;
	function init_shared() { ... }
	//#endregion
	__share_export(exports, {
		n: () => count,
		r: () => init_shared
	});
});
var share_shared = __share_require("shared");
//#region a.js
function init_a() {
	return (init_a = __esmMin((() => {
		share_shared.r();
		globalThis.events.push("A " + share_shared.n);
	})))();
}
//#endregion
init_a();
```

`EcmaGenerator` merges the carried records' `RenderedModule`s into `RenderedChunk.modules`. `RenderedChunk.imports` comes from the projected `cross_chunk_imports`; content hashes include the factory text, and the runtime import carries its placeholder, so hash dependencies are correct without changes to `finalize_chunks.rs`. `renderChunk`, `augmentChunkHash` and `generateBundle` see only files.

## Runtime registry

In `crates/rolldown/src/runtime/runtime-base.js`; flags regenerated into `crates/rolldown_common/src/generated/runtime_helper.rs` by `just update-generated-code`. See `../runtime-helpers/implementation.md` for the state machine.

## Tests

- Rust fixtures: `crates/rolldown/tests/rolldown/function/inline_common_chunks/` — output shape for the behaviours (`basic`, `record_reads_record`, `cycle_in_record_from_{x,y}`, `cjs_member`, `call_forms`, `dynamic_entry_reader`, `record_imports_file`, `carrier_shadows_record_global`, `minify`, `sourcemap`), the `kept_as_file/*` shapes worth pinning (an entry's implementation chunk, a chunk re-exported by a dynamic entry, a retained dynamic import, a static cycle with a file) and the three `errors/*` messages (an invalid `maxSize`, a missing requirement, `experimental.devMode`, which the JS API rejects before validation and so only a Rust fixture can reach).
- Node: `packages/rolldown/tests/behaviors/inline-common-chunks/` — every case builds with the option off and on, runs each entry and dynamic entry as the root of a fresh Node process and compares logs, exports, identities and errors; the harness also parses the `on` output to check that each `__share_require(id)` follows `__share(id, ...)` in the same file and that the runtime chunk is imported first. The kept-as-file rule per candidate and the configuration error per remaining precondition are each one Node case. `registry.test.ts` loads `runtime-base.js` and pins the five registry states.

## Invariants

- A record is never instantiated, named as a file, or listed in another chunk's `cross_chunk_imports`.
- Every record has at least one carrier; every carrier carries the closure of the records it reads.
- A record's module-scope footprint in a carrier is exactly its import bindings.
- Registration precedes any `__share_require` in the same file; the first `__share(id)` wins; a completed, executing or failed record never re-runs its factory.

## Related

- [design.md](./design.md)
- `../code-splitting/implementation.md`
- `../runtime-helpers/implementation.md`
