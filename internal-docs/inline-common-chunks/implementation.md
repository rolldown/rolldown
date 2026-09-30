# Inline Common Chunks — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`output.codeSplitting.experimentalInlineCommonChunks` adds one struct, `InlineCommonChunksState` (`crates/rolldown/src/stages/generate_stage/inline_common_chunks/`), and threads it through the generate stage. Selection and placement happen inside the cross-chunk link step, on the derived final edges; the physical file graph is projected right after the links are committed; naming, module finalization and the ESM renderer treat a carried record's modules as part of the carrying file. Three runtime helpers (`__share`, `__share_require`, `__share_export`) implement the registry.

## Options

| Layer      | Where                                                                                                                                                           | What                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TypeScript | `packages/rolldown/src/options/output-options.ts` (`ExperimentalInlineCommonChunksOptions`), `validator.ts`, `bindingify-output-options.ts`                     | `{ maxSize?: number, exclude?: string \| RegExp \| fn \| Array }`. `exclude` functions are batched into one napi call like `groups[].test`.                                                                                                                                                                                                                                                                                                                                                           |
| Binding    | `crates/rolldown_binding/src/options/binding_output_options/binding_manual_code_splitting_options.rs`                                                           | `BindingInlineCommonChunksOptions`                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Common     | `crates/rolldown_common/src/inner_bundler_options/types/inline_common_chunks_options.rs`                                                                        | `InlineCommonChunksOptions` (raw, deserializable for fixtures) and `NormalizedInlineCommonChunksOptions { max_size: usize, exclude }`; `NormalizedBundlerOptions::inline_common_chunks` is `Some` exactly when `maxSize > 0`; `maxSize: 0` normalizes to `None`.                                                                                                                                                                                                                                      |
| Validation | `crates/rolldown/src/utils/prepare_build_context.rs` (`verify_inline_common_chunks_options`, and the `strictExecutionOrder` default in `prepare_build_context`) | `maxSize` must be a non-negative safe integer or positive `Infinity` (normalized to `usize::MAX`, with no size check). `maxSize > 0` requires `format: 'es'` and an explicit `preserveEntrySignatures: false`, rejects `strictExecutionOrder: false`, requires `preserveModules`, `experimental.devMode` and `experimental.onDemandWrapping` to be off. `prepare_build_context` then turns `strictExecutionOrder` on when the option is omitted. Errors are `InvalidOptionType::InlineCommonChunks*`. |

A fixture's `configVariants` entry with `codeSplitting: { experimentalInlineCommonChunks: { maxSize: 0 } }` snapshots the same input with the option off.

## Pipeline position

```
generate()
  ├─ prepare_inline_common_chunks()          evaluates `exclude` once (async), marks `.d.ts`
  ├─ generate_chunks()                        try_merge_runtime_chunk() returns early while on
  ├─ finalize_chunk_plan()                    order lowering, sweep: no record-specific behavior
  ├─ used_symbol_refs.seal(); compute_wrapped_esm_init_metadata()
  ├─ compute_cross_chunk_link_state()         pure derivation of the final logical edges
  ├─ select_inline_common_chunks()            <- selection point: records, placement, registry demand
  ├─ compute_cross_chunk_link_state()         again, only when something was selected: the demand
  │                                            adds the runtime chunk's `__share*` imports/exports
  ├─ commit_cross_chunk_links()               the single writer of the chunk graph's link fields
  ├─ apply_inline_common_chunks_links()       <- physical projection
  ├─ generate_chunk_name_and_preliminary_filenames()   record id; carriers' moduleIds
  ├─ deconflict_chunk_symbols()               per file, over own + carried modules; per record, for its id
  ├─ finalize_modules()                       own modules in place; finalize_inline_copies() per carrier and per record
  └─ render_chunk_to_assets()                 render_inline_records() prints factories and bridges
```

## State

```rust
pub struct InlineCommonChunksState {
  excluded_modules: FxHashSet<ModuleIdx>,
  records: FxIndexMap<ChunkIdx, InlineRecord>,           // selected chunks, exec order; `id`
  placement: InlinePlacement,                          // readers, carried, reading_files (place.rs)
  names: FxHashMap<ChunkIdx, FileInlineNames>,           // file -> exports param, file bridges, factory bridges
  runtime_chunk: Option<ChunkIdx>,
}
pub struct FileInlineNames {
  pub exports_param: CompactStr,
  pub bridges: FxHashMap<ChunkIdx, CompactStr>,                            // record -> `var share_x`
  pub factory_bridges: FxHashMap<ChunkIdx, FxHashMap<ChunkIdx, CompactStr>>, // carried record -> (record it reads -> binding)
}
pub struct InlineReader { pub file: ChunkIdx, pub reader: ChunkIdx }   // whose names, whose bridges
```

`ChunkGraph` and `Chunk` carry no record-specific fields; that state lives in `InlineCommonChunksState`. A record's `preliminary_filename` and `pre_rendered_chunk` stay empty; filename generation, in-place finalization, `resolve_file_urls`, `detect_ineffective_dynamic_imports` and `instantiate_chunks` skip records. A record is deconflicted over its own modules; that name table serves only the rendering its id hashes.

## Selection (`inline_common_chunks/select.rs`)

Runs on the `CrossChunkLinkState` of the final link derivation: every module is in its final chunk, liveness is sealed, order lowering and the unused-runtime sweep are done. The runtime chunk must exist and be standalone (`try_merge_runtime_chunk` returns early while the option is on; the sweep keeps the runtime whenever a candidate exists, because a candidate's members demand `__esmMin`/`__esm` or `__commonJS`); otherwise nothing is selected. The static importee table is `index_imports_from_other_chunks` plus the edges chunk merging pre-populated on `Chunk::imports_from_other_chunks`, the same union `commit_cross_chunk_links` writes.

`prefers_inline_common_chunk` lets dynamic already-loaded reduction and common-chunk merging preserve small, eligible module groups before entry assignment. It shares `inline_common_chunk_module_bailout` with final selection and ignores runtime modules, whose placement is separate.

The rule list from `design.md` is applied to every live chunk in `sorted_chunk_idx_vec` order (`keep_as_file_reason`). Two rules need whole-graph facts: chunks declaring a symbol another chunk exports (`foreign_exported_owner_chunks`: ownership from the derivation's `symbol_chunk` table, because an order wrapper's CommonJS interop symbols are declared in the CommonJS module's chunk while the importing module owns them, and for an exported namespace alias also the chunk declaring the namespace binding, since the file prints `var x = ns.prop` for it) and chunks whose symbols a direct-`eval` module references (`eval_read_chunks`). `import.meta` is the scanner's per-statement `StmtInfoMeta::ImportMeta` flag, checked only on included statements; an emitted asset is a module the file emitter associates with a file reference (`file_ref_for_module`), as the builtin asset and copy module types do; a star re-export of an external is `star_exports_from_external_modules`, for which the finalizer prints an import declaration inside the module body. After the per-chunk rules, a Tarjan SCC over the static chunk graph (runtime chunk excluded) drops every candidate that shares a component with a non-candidate. The placement loop then drops a record no file carries and a record that a file holding a direct-`eval` module would print (`eval_files`), recomputing the placement until every record passes: a carrier names its own modules and its records' in one pass, and a binding an `eval` reads by its source name must not be renamed.

Selecting registers the registry demand on the reading files: `ShareRequire` for every file that reads a record, plus `Share` and `ShareExport` when it carries one. Records register nothing: their modules' own helper demand (`__esmMin`, `__commonJS`) is on the record chunk's synthetic statements and is taken over by the carriers in the projection. `compute_runtime_symbol_closure` then pulls `__share_factories`, `__share_records`, `__create` and `__defProp` in. `generate()` derives the link state once more so the runtime chunk exports the helpers and the reading files import them; the placement computed from the second derivation's non-runtime edges must equal the first (asserted in the projection).

Every decision is logged: `RD_LOG=rolldown::inline_common_chunks=debug` prints `kept as file` with the rule, `selected as record` with size and readers, and each carrier's takeover.

## Placement (`inline_common_chunks/place.rs`)

`compute_placement` reads static and dynamic importee tables, files with included untracked dynamic imports, liveness, independent entry points and chunk execution order. `readers[c]` lists the records among `c`'s static importees in execution order. Each file needs the depth-first post-order closure of those records. A Tarjan traversal of the projected static graph removes factories that dependencies outside its static cycle already register; `registered[f]` contains the registrations guaranteed when that file's code runs.

`prune_async_inherited_records` derives the dynamic edges of each file and every record it can print after that static pass. Its work queue begins at user-defined entry chunks and `chunk_idx_to_reference_ids` (plugin-emitted files), each with an empty bitset. A static edge propagates the source's incoming bitset; a dynamic edge also adds its `registered` set. Files with included untracked dynamic imports propagate the same dynamic bitset to every physical file because the build cannot identify their targets. The first path discovers a file and each later path intersects its bitset; a changed set requeues that file. The final incoming bitsets remove redundant factories from `carried`. This graph includes all possible dynamic parents even if some copies are then omitted.

`reading_files` lists every file with a non-empty `readers` entry; a file carrying no factory still gets bridges, `__share_require` and the runtime import. A record with no carrier is dropped by selection. The [design](./design.md#registration-on-entry-loading-paths) explains the ordering proof.

## Physical projection (`inline_common_chunks/rewire.rs`)

Right after `commit_cross_chunk_links` wrote `imports_from_other_chunks` and `cross_chunk_imports`:

- `readers`/`carried` are recomputed from the committed edges and must equal the selection-time placement, which the runtime demands were registered from (asserted).
- A record drops its imports of other records and keeps the rest.
- A file that reads records keeps its committed import order and expands every record in place into the record's own committed importees (recursively for records a record reads, each once): the files behind a record, externals included, evaluate where they did before projection. The import items are the file's own plus those of every record it carries, per importee; a record the file reads without carrying it expands into bare imports. The runtime chunk is imported first.
- A file takes over the `cross_chunk_dynamic_imports` of the records it prints, deduplicated with its own. The finalizer rewrites each resolved internal target relative to that file.
- Afterwards no live file has an edge to a record (asserted).

The projection leaves `exports_to_other_chunks` as committed: a record's export names are the getter keys of its exports object, and consumers resolve them like any cross-chunk export name.

## Naming

In `generate_chunk_name_and_preliminary_filenames` a record gets the `[name]` any common chunk would get (its last module's representative name; the `sanitizeFileName` hook does not run for it), and its registry id (`InlineRecord::id`, `record_id`) is that name plus eight characters of the xxhash of its members' stable ids, sorted: `shared-ZmCA4nrZ`. The id is fixed before anything is rendered and printed as it is; it identifies the record within its build and stays the same across builds while the module set does, so a carrier's hash, which covers the copies it prints, moves only with the carrier's own content. Two records of one build whose name and eight characters collide rehash until they differ. The id carries no content hash: files of two builds that meet in one page and import the same runtime file share a registry, and the option does not support that (design.md, principle 10 and the unresolved questions).

## Deconflicting

A file that reads records is deconflicted once, by the ordinary `deconflict_chunk_symbols`, over an `InlineNamingInput` (`inline_common_chunks/naming.rs`): its own modules and every carried record's modules merged in ascending execution order, and the synthetic statements (`init_*` declarations) of its own chunk and of the carried records. The records' top-level symbols therefore get names in the carrier's table exactly like the carrier's own; that is more conservative than the factory scope requires, and either side can get a `$1` suffix it would not have as separate files: a record symbol when the carrier declares the name first, a carrier symbol when a record module that executes later claims it. A function's or class's `.name` can therefore differ from the option-off build, in the record's modules and in the carrier's own (design.md, unresolved questions). The same renamer then names, avoiding the nested-scope bindings of all those modules, one file-level bridge per record the file reads (`share_<name>`), one factory-local bridge per record each carried factory reads, and the factories' `exports` parameter (moved aside when any module in the file reads a global `exports`). A record is deconflicted as a chunk of its own as well, over an input with its modules alone and one bridge per record it reads; that table serves only the rendering its id hashes (`finalize_inline_copies`), which no file prints.

Because the carrier's import table already holds the carried records' import items (projection), one import declaration per importee covers the file and its factories, and a symbol both need is one binding.

## Consumer references (module finalizer)

`ScopeHoistingFinalizerContext` carries `file_idx` (the file the code is printed in, whose `canonical_names` the finalizer reads through `chunk`) separately from `chunk_idx` (the chunk the module is placed in); together they form the `InlineReader` a bridge lookup needs. `InlineCommonChunksState::bridge_read(reader, canonical_ref, ..)` answers `Some((bridge, export_name))` when the symbol's chunk (`symbol_db.get(ref).chunk_idx`, committed by the link pass) is a record other than `reader`, using the file's bridges for a file and the factory's bridges for a record printed in it; `ScopeHoistingFinalizer::inline_bridge_member` and `GenerateContext::inline_bridge_pattern` both go through it. Hooks:

- `finalized_expr_for_symbol_ref` renders `bridge.export_name` and, when the expression is a callee, `(0, bridge.export_name)`, the same guard namespace-member callees use. A call the finalizer marks pure gets the same guard. The wrapper calls the finalizer prints itself (`init_x()`, `require_x()`) stay bare: the `__esm`/`__commonJS` wrappers never read `this`, and the callee guard below knows which bridge members are wrappers.
- `visit_expression` adds the guard for the forms the callee flag does not cover: a member callee that resolves to exactly a bridge member (`ns.f()`), the callee of an optional call inside a `ChainExpression` (`f?.()`, `ns.f?.()`), a tagged-template tag, and in each of these positions a parenthesized optional member (`(ns?.f)()`, `(ns?.f)?.()`, `` (ns?.f)`x` ``). `try_rewrite_bridge_callee` rewrites these shapes; `callee_guard.rs` fails the build on any of them left bare after finalization.
- `wrapper_is_reachable_in_chunk` counts a bridged wrapper as reachable, so `init_*()` calls into a record are emitted as `bridge.init_x()`.
- `try_rewrite_cjs_member_expr_assignment_target` rewrites an assignment to a property of a CommonJS default import (`lib.count = 1`, `lib.count++`, `lib['k'] = v`) to `ns.default.prop`, and `ns` is the bridge member when a record owns the namespace binding.
- `canonical_name_for` asserts in every build that a symbol owned by a record is never printed by name from another chunk: a carrier's name table holds the record's names too, so a reference that skipped the bridge would name a binding that exists only inside the factory, or a same-named binding of the carrier.
- `GenerateContext::finalized_string_pattern_for_symbol_ref` (entry prologues, the getter table) takes the same path, with the file as `chunk_idx` and the reader as its `cur_chunk_idx` argument.

`finalize_modules` skips a record's modules in the in-place pass. `finalize_inline_copies` (`inline_common_chunks/finalize.rs`) then finalizes them once per carrier: for each carrier, for each carried record, for each module, `EcmaAst::clone_with_another_arena` (semantic ids preserved), a finalizer context with the carrier's `canonical_names` and `InlineReader { file: carrier, reader: record }`, and `NormalModule::render` with one level of indentation. The results (`CarriedRender { record, modules: Vec<ModuleRenderOutput> }`) are kept per carrier until the files are instantiated. Errors from every carrier's copy, the declaration check's and the callee guard's included, fail the build; the finalizer's warnings are taken from a record's first carrier only. A module whose finalized body still holds an import or export declaration (a shape no selection rule caught) fails the build with an error naming the module, instead of printing a syntax error into the carrier. `render_chunk_exports` asserts in every build that a file exports no symbol a record declares.

## Rendering

`instantiate_chunks` hands each file its `CarriedRender`s. `EcmaGenerator` turns them into `RenderedModuleSource`s through the same `render_ecma_module` as the file's own modules (source maps, `//#region` markers), adds them to `RenderedChunk.modules`, and `render_esm` prints, after the file's imports and before anything else (`share_factory.rs::render_inline_records`):

```js
import { i as __share_require, n as __share, r as __share_export, t as __esmMin } from "./rolldown-runtime.js";
__share("shared-ZmCA4nrZ", (exports) => {
	//#region shared.js
	var count;
	function init_shared() { ... }
	//#endregion
	__share_export(exports, {
		n: () => count,
		r: () => init_shared
	});
});
var share_shared = __share_require("shared-ZmCA4nrZ");
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

Factories come in `carried` order (dependencies first); a factory that reads other records opens with `var share_y = __share_require("<id>")` for each. `RenderedChunk.imports` comes from the projected `cross_chunk_imports`; content hashes include the factory text and the record ids; the runtime import carries its placeholder. `renderChunk`, `augmentChunkHash` and `generateBundle` see only files, and each carrier lists the carried modules after its own in `moduleIds` and in execution order in `modules`: a record's module belongs to every file that prints it.

## Runtime registry

In `crates/rolldown/src/runtime/runtime-base.js`; flags regenerated into `crates/rolldown_common/src/generated/runtime_helper.rs` by `just update-generated-code`. See `../runtime-helpers/implementation.md` for the state machine.

## Tests

- Rust fixtures: `crates/rolldown/tests/rolldown/function/inline_common_chunks/` pin the output shape (`basic`, `dynamic_import_member`, `record_reads_record`, `cycle_in_record_from_x`, `cjs_member`, `call_forms`, `dynamic_entry_reader`, `record_imports_file`, `carrier_shadows_record_global`, `carrier_names_collide`, `two_records_share_import`, `record_reads_global_exports`, `inherited_from_dependency`, `minify`, `sourcemap`, `inherited_through_projection`), the `kept_as_file/*` shapes (an entry's implementation chunk, a chunk re-exported by a dynamic entry, a static cycle with a file, a member that is an emitted asset) and the `errors/*` messages (an invalid `maxSize`, a missing requirement, `strictExecutionOrder: false`, `experimental.devMode`, which the JS API rejects before validation and so only a Rust fixture can reach). `basic`, `cjs_member`, `record_imports_file` and `record_reads_record` also build with `maxSize: 0` (a `configVariants` entry named `off`), so each snapshot holds the output with the option off next to the output with it on. `compute_placement` has unit tests in `place.rs`.
- Node: `packages/rolldown/tests/behaviors/inline-common-chunks/`. The suite covers unlimited size, removed `import.meta`, dynamic-loader target paths and hashes, main/lazy placement, and untracked relative imports in different output directories. The differential cases build with the option off and on, run each configured and plugin-emitted entry as the root of a fresh Node process and compare logs, exports, identities and errors; the harness also checks local registration order and that the runtime chunk is imported first; executed paths check factory availability through the runtime. `registry.test.ts` loads `runtime-base.js` and pins the five registry states. `inline-common-chunks-fuzz.test.ts` generates module graphs and option sets from a seed (`tests/src/inline-common-chunks/fuzz.ts` lists the shapes) and runs the same off/on comparison; `INLINE_COMMON_CHUNKS_FUZZ_SEEDS` raises the seed count.

## Invariants

- A record is never instantiated, named as a file, or listed in another chunk's `cross_chunk_imports` (asserted after projection); its own name table serves only the rendering its id hashes.
- The placement computed at selection equals the one computed from the committed edges (asserted), which leaves every record with at least one carrier; a file carries the closure of the records it reads minus the registrations guaranteed by its static dependencies and entry loading paths.
- A record id follows the record's module set; a file's hash covers the copies and ids it prints, so a hashed file that keeps its name keeps its bytes.
- Every name a factory uses comes from the carrier's own renamer, and every import a factory needs is in the carrier's own import table. A record's symbol is printed by name only inside its own chunk; every other chunk reads it through a bridge (asserted in `canonical_name_for`), and a bridge member other than a module wrapper is never a bare callee or template tag: every other call through a bridge is printed as `(0, bridge.name)(...)`, so `this` stays `undefined` as it is with the option off (`inline_common_chunks/callee_guard.rs` checks every finalized module of a reading file and every record copy, wrappers identified by symbol against the record's export table; a miss fails the build, from whichever carrier's copy it comes).
- A registration precedes any `__share_require` that can see it, earlier in the same file, in a static dependency that has already run, or on every prior entry loading path; the first `__share(id)` wins; a completed, executing or failed record never re-runs its factory.

## Related

- [design.md](./design.md)
- `../code-splitting/implementation.md`
- `../runtime-helpers/implementation.md`
