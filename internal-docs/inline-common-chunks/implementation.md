# Experimental inline common chunks: implementation blueprint

This document describes the implemented `output.experimentalInlineCommonChunks` feature in
Rolldown's current chunking architecture. It states the public behavior, the evidence behind design
choices, the owning data structures, the stage-to-stage contracts, the concrete file changes, and
the validation boundary. [design.md](./design.md) is the shorter rationale.

## 1. Behavior and decisions

### Public contract

The option is an output option:

```ts
interface ExperimentalInlineCommonChunksOptions {
  /** Source bytes; chunks strictly below this value may be selected. */
  maxSize?: number;
}

interface OutputOptions {
  experimentalInlineCommonChunks?: ExperimentalInlineCommonChunksOptions;
}
```

Example:

```js
const bundle = await rolldown({
  input: { pageA: './page-a.js', pageB: './page-b.js' },
  preserveEntrySignatures: false,
});

await bundle.write({
  format: 'es',
  experimentalInlineCommonChunks: { maxSize: 10 * 1024 },
});
```

`maxSize` uses the sum of the transformed module source sizes in the logical chunk. Final rendered
size is unavailable when placement is decided. An omitted option, omitted `maxSize`, non-finite
value, or value at or below zero disables selection. The disabled path does not enable strict order
or change output; a Node API regression test compares `maxSize: 0` with an omitted option
byte-for-byte.

When enabled, Rolldown:

1. finds automatic common chunks whose estimated size is below `maxSize`;
2. chooses carrier chunks that are guaranteed to register each selected factory before use;
3. keeps the logical chunk for symbol ownership but omits its output file;
4. copies one scope-hoisted factory definition into each carrier;
5. replaces static binding references across that boundary with reads from one cached exports
   object; and
6. reports every physical copy in plugin-visible chunk metadata.

The factory can occur in several files, but its registry key resolves to one record per loaded ESM
runtime instance. A stateful shared module therefore initializes once and keeps one state object even
when independently loaded entries contain different physical copies.

### Required option combination

Positive finite `maxSize` accepts only this initial configuration:

| Option                          | Required behavior | Handling                                               |
| ------------------------------- | ----------------- | ------------------------------------------------------ |
| `format`                        | ES output         | Other formats are rejected.                            |
| `codeSplitting`                 | Enabled           | `false` is rejected.                                   |
| `preserveModules`               | `false`           | `true` is rejected.                                    |
| input `preserveEntrySignatures` | `false`           | Every other mode, including the default, is rejected.  |
| `strictExecutionOrder`          | Wrap all          | Omission becomes `true`; explicit `false` is rejected. |
| `experimental.onDemandWrapping` | Disabled          | `true` is rejected.                                    |

Requiring `preserveEntrySignatures: false` follows the narrow first contract agreed in
[backlog #15](https://github.com/rolldown/backlog/issues/15). Supporting entry export surfaces later
requires a complete rule for facades, native re-exports, and property-backed live bindings.

### Selection boundary

`select_inline_common_chunks` starts with live `ChunkKind::Common` chunks whose
`ChunkReasonType::Common` identifies automatic code splitting. A candidate remains a normal output
file in any of these cases:

| Exclusion                                                          | Reason                                                                 |
| ------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Runtime chunk                                                      | The registry must have one physical instance.                          |
| Manual group or manual `maxSize` split                             | User-directed output remains authoritative.                            |
| Plugin-emitted chunk                                               | Its requested output identity must remain a file.                      |
| Dynamic-import target                                              | Dynamic entry identity and namespace loading still require a file.     |
| Contains a dynamic import                                          | The rendered specifier is relative to the carrier file.                |
| Contains `import.meta`                                             | Its identity would become the physical carrier rather than the source. |
| Contains or depends on top-level await                             | The registry factory protocol is synchronous.                          |
| Has an external dependency                                         | Moving it can change relative paths or side-effect order.              |
| Statically depends on a chunk that is not another candidate        | Its import cannot yet be merged into every carrier in source order.    |
| Contains direct `eval`, or is directly consumed by such a chunk    | Opaque strings can observe lexical bindings added around the boundary. |
| Has an unresolved reference matching a factory protocol name       | Wrapping would capture the reference as a factory parameter.           |
| It owns a cross-chunk export that would require a native re-export | Native ESM cannot re-export a runtime property read as a live binding. |
| Size is at least `maxSize`                                         | The option selects only chunks below the threshold.                    |
| Has no static importer or receives no carrier                      | Omitting the file would lose reachable code.                           |

Candidate-to-candidate static dependencies are supported and become registry requires. The one
allowed non-candidate dependency is the standalone registry/runtime chunk, whose helper imports are
rendered relative to each carrier.

### Evidence and decisions

The implementation uses these public discussions as constraints:

- The [RFC](https://github.com/rolldown/rolldown/discussions/10693) defines physical duplication,
  one logical module record, live bindings, side-effect order, a source-size threshold, and the
  initial top-level-await exclusion.
- [Backlog #7](https://github.com/rolldown/backlog/issues/7) chooses per-chunk factories rather than
  per-module factories and requires strict execution order.
- [Backlog #12](https://github.com/rolldown/backlog/issues/12) requires short, unique, build-stable
  chunk keys. Hashing the logical set of cwd-relative, slash-normalized `StableModuleId` values
  satisfies that without coupling consumers to a content hash or checkout location.
- [Backlog #13](https://github.com/rolldown/backlog/issues/13) requires downstream execution-order
  effects to participate in placement. This implementation reasons over the complete static chunk
  graph and does not inherit placement coverage within a strongly connected component.
- [Backlog #14](https://github.com/rolldown/backlog/issues/14) records strict order as mandatory.
- [Backlog #17](https://github.com/rolldown/backlog/issues/17) leaves registration-before-require as
  an open design problem. Local placement plus dependency-only redundant-placement elimination is
  the conservative implemented answer.
- [Backlog #9](https://github.com/rolldown/backlog/issues/9) identifies plugin, CSS, and declaration
  output risks when a module appears in several chunks. The implementation fixes core chunk metadata
  but does not claim ecosystem-wide compatibility.

### Verified behavior and remaining limits

Scenario tests verify multiple entries, one-time stateful evaluation, live bindings, plain-function
`this` for direct, optional, and tagged-template calls, module cycles inside an inlined chunk,
namespace identity, CommonJS export identity, chained inlined chunks, side-effect order,
cross-chunk re-exports, minified output, source maps, stable keys across content and root-directory
changes, plugin metadata, unusual filenames, and conservative dependency/dynamic/manual/TLA
behavior.

The remaining limits are explicit:

- CJS, IIFE, and UMD output are unsupported even though CommonJS _input modules_ in ES output are
  tested.
- Native entry signatures other than `false`, preserve-modules output, and on-demand wrapping are
  unsupported.
- Top-level await, dynamic imports, `import.meta`, direct `eval`, external dependencies,
  generated-protocol-name globals, and dependencies on non-selected chunks are left in files, not
  inlined.
- Cycles among separate inlined chunks use a pre-cached partial exports record and are best-effort;
  the tests cover cycles among modules inside one inlined chunk, which keep Rolldown's normal strict
  wrapper behavior.
- Core plugin metadata is tested. Vite CSS extraction, declaration generation, HMR/dev output, and
  third-party plugins that assume one placement per module have not been certified.

## 2. Layers and dependencies

The implementation follows the current direction of dependencies: public JavaScript configuration
flows into bindings and common option types; generate-stage policy reads linked chunk data; module
finalization reads the resulting chunk annotations; rendering consumes finalized modules and emits
assets.

```text
TypeScript OutputOptions
        │
        ▼
N-API binding object ──► BundlerOptions validation/normalization
        │                         │
        │                         ▼
        │                  NormalizedBundlerOptions
        │                         │
        ▼                         ▼
link output + ChunkGraph ─► inline selection / placement / graph rewiring
                                      │
                        ┌─────────────┼─────────────┐
                        ▼             ▼             ▼
                  deconfliction   finalization   two-phase rendering
                        │             │             │
                        └─────────────┴──────┬──────┘
                                             ▼
                              chunks, source maps, plugin metadata
```

| Layer                      | May depend on                         | Data crossing its boundary                                              |
| -------------------------- | ------------------------------------- | ----------------------------------------------------------------------- |
| TypeScript API             | Public option types and validators    | `experimentalInlineCommonChunks: { maxSize }`                           |
| N-API binding              | Binding structs and core option types | Optional `f64` threshold                                                |
| Common options             | No generate-stage implementation      | Raw and normalized option value; enable/threshold helpers               |
| Option preparation         | Raw options and diagnostics           | Validated combination; strict-order default                             |
| Linking/chunk graph        | Modules, symbols, chunk ownership     | Final live chunks, imports, export ownership, execution order           |
| Inline selection           | Linked data and current `ChunkGraph`  | `InlinedCommonChunks` plus placement annotations on `Chunk`             |
| Deconfliction/finalization | Chunk annotations and symbol database | Collision-free names and property-backed cross-boundary expressions     |
| ESM rendering              | Finalized modules and source maps     | Registry, factories, requires, and remaining files                      |
| Plugin/asset stages        | Rendered chunks and rewired graph     | Truthful module ledgers, imports, hashes, source maps, and output files |

The current architecture creates one necessary cross-layer responsibility: `Chunk` is both the
logical link owner and the unit later rendered as a file. The feature cannot replace that ownership
without invalidating `module_to_chunk`, symbol-to-chunk links, init obligations, and finalizer
lookups. The selection pass therefore annotates logical chunks and carriers, while the renderer
alone decides that an annotated logical chunk emits no file. The later refactor should separate
logical ownership from physical placement directly.

## 3. Responsibilities and ownership

| Owner                                        | Layer                     | Owns or decides                                                                               | Consumers                                               |
| -------------------------------------------- | ------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `ExperimentalInlineCommonChunksOptions`      | Common options            | Raw `max_size`                                                                                | Validation and normalized helpers                       |
| `NormalizedBundlerOptions`                   | Common options            | Whether selection is enabled and the finite positive threshold                                | Runtime retention and inline selection                  |
| `prepare_build_context`                      | Option preparation        | Compatibility errors and automatic strict-order enablement                                    | All later stages                                        |
| `GenerateStage::select_inline_common_chunks` | Generate policy           | Candidate selection, registry owner, keys, carriers, required factories, and graph rewiring   | Naming, finalization, rendering                         |
| `InlinedCommonChunks`                        | Generate policy           | Ordered logical chunks that emit factories and the registry chunk                             | Deconfliction and two-phase rendering                   |
| Logical inlined `Chunk`                      | Chunk graph               | Module ownership, exports, canonical names, stable registry key, and required child factories | Finalizer and factory renderer                          |
| Carrier `Chunk`                              | Chunk graph               | `carried_inline_chunks` and its local exports-object bindings                                 | Deconfliction, ESM renderer, plugin metadata            |
| `deconflict_chunk_symbols`                   | Naming                    | Names for logical bodies and consumer bindings; generated-name reservations                   | Finalizer and renderer                                  |
| `ScopeHoistingFinalizer` / `GenerateContext` | Module finalization       | Rewriting cross-boundary symbol references to property access                                 | Rendered module bodies and chunk exports                |
| `InlinedChunkRender`                         | Rendering                 | One rendered logical factory, body map, and module ledger                                     | Every physical carrier                                  |
| `share_factory`                              | ESM format                | Registry code, factory interface, registration and require prologues                          | Emitted ES chunks                                       |
| `EcmaGenerator`                              | Rendering/plugin boundary | Carrier `RenderedChunk.modules` and `moduleIds`                                               | `renderChunk`, `augmentChunkHash`, and `generateBundle` |

## 4. Concrete data structures

The sketches below omit unrelated fields but use the implemented names and types.

### Options

```rust
// crates/rolldown_common/.../experimental_inline_common_chunks_options.rs
pub struct ExperimentalInlineCommonChunksOptions {
  pub max_size: Option<f64>,
}

pub struct BundlerOptions {
  pub experimental_inline_common_chunks: Option<ExperimentalInlineCommonChunksOptions>,
  // ...
}

pub struct NormalizedBundlerOptions {
  pub experimental_inline_common_chunks: Option<ExperimentalInlineCommonChunksOptions>,
  pub strict_execution_order: bool,
  // ...
}
```

`prepare_build_context` copies the validated value into `NormalizedBundlerOptions`. The helper
`inline_common_chunks_max_size()` returns a finite positive `f64` or zero;
`is_inline_common_chunks_enabled()` is the only boolean policy reader.

### Selected chunks and annotations

```rust
pub struct InlinedCommonChunks {
  /// Logical chunks replaced by factories, ordered by execution order.
  pub chunks: Vec<ChunkIdx>,
  /// Standalone chunk that owns the runtime module and registry.
  pub registry_chunk: Option<ChunkIdx>,
}

pub struct Chunk {
  /// `Some` means this logical chunk emits factories instead of a file.
  pub inline_share_key: Option<ArcStr>,
  /// Logical inlined chunks whose factory body is copied into this file.
  pub carried_inline_chunks: Vec<ChunkIdx>,
  /// Logical inlined chunks this chunk executes through the registry.
  pub required_inline_chunks: Vec<ChunkIdx>,
  /// Consumer-local exports-object binding for each required logical chunk.
  pub inline_binding_names_for_other_chunks: FxHashMap<ChunkIdx, String>,
  /// Conflictless local names for registry definitions or imports in a physical chunk.
  pub inline_share_define_name: Option<String>,
  pub inline_share_require_name: Option<String>,
  // ...existing logical ownership, imports, exports, and canonical names...
}
```

The selection pass writes these fields after cross-chunk links exist. No module changes its
`module_to_chunk` owner. Deconfliction fills `inline_binding_names_for_other_chunks`; finalization
and rendering read the resulting placement annotations.

`inline_share_key` is the URL-safe xxHash of a deterministic identity string built from sorted,
length-prefixed `StableModuleId` values. Those IDs are cwd-relative and slash-normalized. If two
identity strings ever collide within one output, a deterministic numeric suffix distinguishes them.

### Rendering handoff

```rust
pub struct InlinedChunkRender {
  /// Complete `__rd_share(key, factory)` source.
  pub factory: String,
  /// Location of the logical body inside `factory`.
  pub body_range: Range<usize>,
  /// Source map whose destination starts at the body's first line.
  pub body_map: Option<SourceMap>,
  /// Plugin-visible module details copied into every carrier.
  pub rendered_modules: FxHashMap<ModuleId, RenderedModule>,
}

pub struct GenerateContext<'a> {
  pub inline_renders: &'a FxHashMap<ChunkIdx, InlinedChunkRender>,
  pub inline_registry_chunk: Option<ChunkIdx>,
  // ...
}
```

Phase A creates `InlinedChunkRender` values. Phase B passes the map to each normal chunk renderer.
When source maps are enabled, the carrier splits `factory` at `body_range` and appends a cloned body
map between the unmapped generated prefix and suffix. The same logical source is therefore mapped in
every physical carrier.

### Emitted registry record

The generated JavaScript record has this effective shape:

```ts
interface SharedRecord {
  module: { exports: Record<string, unknown> };
  failed: boolean;
  error: unknown;
}
```

`__rd_share_require` stores the record before invoking its factory, so re-entry sees partial exports.
It records failure separately from `error` because JavaScript can `throw undefined`; every later
require rethrows the same failed evaluation.

## 5. Processing stages and data flow

### Stage 0: validate and normalize

`bindingifyOutputOptions` passes the public object into
`BindingExperimentalInlineCommonChunksOptions`, and `normalize_binding_options` creates the core
option. `verify_raw_options` checks the configuration only when `maxSize` is finite and positive.
Normalization then uses that same predicate to default `strict_execution_order` to `true`.

Invariant: validation and normalization must use the same enabled predicate. Otherwise an invalid
combination could bypass errors but still activate selection, or validation could reject a disabled
configuration.

### Stage 1: start from final logical cross-chunk links

The selection pass runs immediately after `compute_cross_chunk_links`. At this point it can read:

- the final live static and dynamic chunk edges;
- `imports_from_other_chunks` and `exports_to_other_chunks`;
- canonical symbol ownership (`SymbolRefDataClassic::chunk_idx`);
- chunk execution order and reason; and
- module metadata for top-level await, dynamic imports, and `import.meta`.

It must run before deconfliction and module finalization because both stages need the new boundary.

### Stage 2: select logical chunks

Selection applies the table in section 1. It also scans the final chunk export tables before choosing
candidates: if another chunk must natively re-export a symbol owned by candidate `S`, `S` stays a
file. Scope-hoisted internal re-exports that canonicalize directly to `S` do not need such an output
re-export; their consumers read `S`'s cached exports object. The cross-chunk re-export scenario test
covers that shape with separate ABC and AB factories.

Candidate closure is then reduced to a fixed point: every non-runtime static dependency must itself
remain a candidate, and a candidate directly imported by a direct-eval chunk is removed. This
avoids moving an import across a host's other dependencies and prevents opaque eval strings from
observing injected bindings. Each selected chunk receives a stable `inline_share_key`. The ordered
selected list becomes `InlinedCommonChunks::chunks`.

### Stage 3: place factories

The selection pass builds `imports: importer -> importees` over all live static chunks.

For every live chunk `X`, `reach(X)` is the transitive set of selected chunks reachable through only
selected edges. A non-selected chunk that needs a selected chunk must carry enough factories to make
that closure executable.

`petgraph::algo::tarjan_scc` condenses the complete static graph into strongly connected components.
Components are processed dependencies first:

1. coverage from a non-selected dependency in a strictly lower component is guaranteed available;
2. coverage from a selected dependency is not inherited, because that dependency no longer emits a
   file;
3. coverage is not inherited within the same component; and
4. `carried[X] = reach(X) - inherited(X)`.

The selection pass asserts that every selected chunk has a carrier and retains a release-mode guard
that keeps an uncarried candidate as a normal file instead of discarding reachable code.

Invariant: every `required_inline_chunks` key has a registration in the same chunk or in a static
dependency that completes before the consumer's component begins.

### Stage 4: rewire the physical file graph

The logical selected chunks stay in `chunk_table`, but normal files stop importing them:

- static imports targeting selected chunks are removed;
- symbol-import entries targeting selected chunks are removed;
- a carrier inherits each carried factory's registry/runtime dependency;
- each carrier or consumer imports the registry chunk; and
- selected-to-selected dependencies become `required_inline_chunks`, not file edges.

Static imports are deduplicated and sorted by execution order after rewiring.

Invariant: no emitted chunk has a file import whose target has `inline_share_key.is_some()`.

### Stage 5: assign collision-free names

The logical factory body is finalized once and copied verbatim, so its free binding names must be
valid in every carrier. Deconfliction proceeds in two parts:

1. inlined chunks are named first in deterministic order; co-hosted factories reserve names already
   chosen by their peers; and
2. every normal chunk reserves the complete canonical-name set of each factory it carries.

Reservations are scoped to generated code that is actually present. Logical factory bodies reserve
the four protocol parameters. Carriers reserve copied bodies' canonical and unresolved names, so a
host declaration cannot capture what was a global lookup in the original common file. Carriers and
consumers choose conflictless local aliases for the two registry imports; the registry chunk keeps
the exported spellings and also reserves its internal tables and helper. Unrelated emitted chunks
reserve none of these names. `inline_binding_names_for_other_chunks` is then assigned with the
existing renamer.

This sequential naming is conservative: it can make names longer and reduces parallelism, but avoids
re-finalizing a body independently for each carrier.

### Stage 6: finalize property-backed references

The scope-hoisting finalizer asks whether a canonical symbol belongs to a different chunk whose
`inline_share_key` is set. If so, it emits `<local exports object>.<chunk export name>` rather than an
identifier imported from a file.

The factory publishes chunk exports with getters. A consumer therefore observes a reassigned export
at read time. If the property is used as a direct or optional call target, or as a tagged-template
tag, finalization emits `(0, object.fn)` so invocation retains ESM's plain-function
`this === undefined` behavior.

Strict wrapper-init routing uses the same property-backed lookup, so a wrapper published by an
inlined chunk is still considered reachable when the import statement is finalized.

Invariant: every cross-boundary symbol read has both a local exports-object binding and a published
getter name.

### Stage 7: render in two phases

`instantiate_chunks` first renders all logical inlined chunks. Each result contains:

- the complete factory source;
- the source map for the factory body;
- rendered-module metadata.

It then renders normal chunks in parallel. ESM rendering emits, in order:

1. ordinary chunk imports;
2. the registry import, unless this is the registry chunk;
3. the registry implementation in its owning chunk;
4. carried factories' runtime-helper imports, resolved from the carrier's directory;
5. factory registrations;
6. `__rd_share_require` bindings; and
7. the carrier's normal finalized body and exports.

Logical inlined chunks are skipped when assets are instantiated. Their module IDs are added before
each physical carrier's filename callback runs, their module metadata is merged before plugin render
hooks receive `RenderedChunk`, and the registry chunk's generated exports are included in
pre-rendered, rendered, and final chunk metadata. The omitted logical chunk itself receives only an
internal naming/path anchor: it does not invoke filename or sanitization callbacks and does not
reserve a real output name.

Invariant: a factory body is rendered and finalized exactly once, while its mapped source and module
ledger can have several physical placements.

### Stage 8: final assets

The existing hash, filename, `renderChunk`, `augmentChunkHash`, and `generateBundle` stages operate on
the rewired emitted chunks. Since factories are already in carrier code and carried modules are in
their ledgers, content hashes and plugin observations include the physical duplication. No output
asset is produced for the logical inlined chunk.

## 6. Concrete file changes

The implementation spans these files:

| Path                                                                                                  | Implemented responsibility                                                      |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `packages/rolldown/src/options/output-options.ts`                                                     | Public experimental option and user-facing contract.                            |
| `packages/rolldown/src/index.ts`                                                                      | Exports the public option interface from the package root.                      |
| `packages/rolldown/src/utils/validator.ts`                                                            | Strict JavaScript option shape validation.                                      |
| `packages/rolldown/src/utils/bindingify-output-options.ts`                                            | Sends the option to N-API.                                                      |
| `packages/rolldown/src/binding.d.cts`                                                                 | Generated N-API TypeScript declaration.                                         |
| `crates/rolldown_binding/src/options/binding_output_options/mod.rs`                                   | N-API option object.                                                            |
| `crates/rolldown_binding/src/utils/normalize_binding_options.rs`                                      | Converts the binding option to the core type.                                   |
| `crates/rolldown_common/src/inner_bundler_options/types/experimental_inline_common_chunks_options.rs` | Core option struct.                                                             |
| `crates/rolldown_common/src/inner_bundler_options/{mod.rs,types/mod.rs}`                              | Raw option field and module registration.                                       |
| `crates/rolldown_common/src/inner_bundler_options/types/normalized_bundler_options.rs`                | Normalized field plus threshold/enable helpers.                                 |
| `crates/rolldown_common/src/lib.rs`                                                                   | Public core type export.                                                        |
| `crates/rolldown/src/utils/prepare_build_context.rs`                                                  | Compatibility validation and automatic strict-order enablement.                 |
| `crates/rolldown_error/src/build_diagnostic/events/invalid_option.rs`                                 | Specific incompatible-option diagnostic.                                        |
| `crates/rolldown_common/src/chunk/mod.rs`                                                             | Logical key, carrier, requirement, and exports-binding annotations.             |
| `crates/rolldown_common/src/ecmascript/ecma_view.rs`                                                  | Records physical-module-sensitive dynamic imports and `import.meta`.            |
| `crates/rolldown/src/ast_scanner/impl_visit.rs`                                                       | Sets the dynamic-import and `import.meta` usage bits during scanning.           |
| `crates/rolldown/src/stages/generate_stage/inline_common_chunks.rs`                                   | Selection, stable keys, SCC placement, carrier guard, and graph rewiring.       |
| `crates/rolldown/src/stages/generate_stage/mod.rs`                                                    | Pipeline insertion, physical pre-render metadata, and naming schedule.          |
| `crates/rolldown/src/stages/generate_stage/{chunk_optimizer.rs,runtime_module_sweep.rs}`              | Keeps the registry's runtime chunk standalone and live.                         |
| `crates/rolldown/src/utils/chunk/deconflict_chunk_symbols.rs`                                         | Generated-name reservations and exports-object binding names.                   |
| `crates/rolldown/src/module_finalizers/{mod.rs,impl_visit_mut.rs}`                                    | Property-backed symbol expressions and plain-call `this` preservation.          |
| `crates/rolldown/src/types/generator.rs`                                                              | Passes rendered factories and registry ownership into ESM generation.           |
| `crates/rolldown/src/utils/chunk/mod.rs`                                                              | Adds carried module IDs and generated registry exports to chunk metadata.       |
| `crates/rolldown/src/ecmascript/format/share_factory.rs`                                              | Registry, factory exports, carrier prologues, and mapped body placement.        |
| `crates/rolldown/src/ecmascript/format/{mod.rs,esm.rs}` and `crates/rolldown/src/ecmascript/mod.rs`   | ESM integration and carrier-relative runtime-helper imports.                    |
| `crates/rolldown/src/stages/generate_stage/render_chunk_to_assets.rs`                                 | Two-phase rendering and omission of logical inlined assets.                     |
| `crates/rolldown/src/ecmascript/ecma_generator.rs`                                                    | Merges carried module ledgers into plugin-visible chunk metadata.               |
| `crates/rolldown_testing/_config.schema.json`                                                         | Generated fixture schema for the core option.                                   |
| `crates/rolldown/tests/rolldown/function/experimental/inline_common_chunks/`                          | Runtime, output, exclusion, minification, and source-map scenarios.             |
| `packages/rolldown/tests/behaviors/experimental-inline-common-chunks.test.ts`                         | Public API validation, key stability, default equivalence, and plugin metadata. |
| `packages/rolldown/tests/cli/__snapshots__/cli-e2e.test.ts.snap`                                      | Records the experimental nested option in generated CLI help.                   |
| `internal-docs/inline-common-chunks/{design.md,implementation.md}`                                    | Rationale and this standalone blueprint.                                        |
| `internal-docs/code-splitting/{design.md,implementation.md}`                                          | Links the late physical-placement policy into the existing architecture docs.   |

## 7. Validation and architectural tradeoffs

### Recorded validation run

The final local run on 2026-09-16 produced these results:

| Command or gate                                                                             | Result                                                                                                                                                    |
| ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `just build-rolldown`                                                                       | Passed; rebuilt the native debug binding and JavaScript/type glue.                                                                                        |
| Focused Rust integration filter `inline_common_chunks__`                                    | Passed all 20 scenario fixtures.                                                                                                                          |
| Focused Node behavior file                                                                  | Passed all 11 API, metadata, identity, stability, and validation tests.                                                                                   |
| `vp run --filter rolldown-tests test:main`                                                  | Passed 46 files and 966 tests; 15 tests were skipped by the existing suite.                                                                               |
| `vp run --filter rolldown-tests test:watcher`                                               | Passed both files and 42 tests; one existing test was skipped.                                                                                            |
| `cargo check --workspace --all-features --all-targets --locked`                             | Passed.                                                                                                                                                   |
| `cargo clippy --workspace --all-targets -- --deny warnings`                                 | Passed.                                                                                                                                                   |
| `cargo fmt --all -- --check`, `just lint-repo`, targeted `vp check`, and `vp run lint-knip` | Passed. `knip` emitted one non-failing pre-existing configuration hint.                                                                                   |
| `just test-rust`                                                                            | 106 unit tests passed; integration reached 2,025 passed and 68 ignored. Its sole failure was the absent `test262/test/language/module-code` submodule.    |
| Full `vp check`                                                                             | Formatting passed for 6,621 files; type checking stopped on the unbuilt `@rolldown/browser` package (two missing-module errors and three cascading anys). |
| `vp run lint-publint`                                                                       | Environment-blocked because `packages/browser/dist` and `packages/debug/dist` artifacts are absent.                                                       |

The browser package artifacts could not be produced in this checkout because its build requires the
uninstalled Rust target `wasm32-wasip1-threads`. The three blocked full-repository observations are
prerequisite/environment failures, not failures in the feature-specific or native Rolldown checks.

### Scenario matrix

| Risk or contract                                           | Test or observation                                                                                                         |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Multiple entries, one stateful instance, no common request | `basic_stateful`; imports two entries sequentially, observes one init and values 1 then 2, and verifies no shared file.     |
| Plain-function `this` and live reassigned exports          | `basic_stateful` and `call_this_semantics`; cover direct, optional, and tagged calls plus live getter reads.                |
| Minifier compatibility                                     | `basic_stateful` `minified` variant executes the minified output and finds two physical key placements.                     |
| Several selected chunks and dependency order               | `chained_common_chunks`; three reachability sets initialize base, AB, and AC once in source order.                          |
| Module cycle, re-export, namespace identity                | `reexport_cycle_namespace`; executes a cycle inside one factory and compares namespace objects across entries.              |
| Re-export across two logical chunk boundaries              | `cross_chunk_reexport`; separate ABC owner and AB barrel factories preserve the owner's live bindings.                      |
| CommonJS input identity in ES output                       | `commonjs_identity`; two entries receive the same mutable `module.exports` object and execute it once.                      |
| Failed evaluation is cached                                | `factory_failure_cached`; two entries observe one evaluation and the same `undefined` failure state.                        |
| Dynamic target remains a file                              | `dynamic_target_skipped`; two dynamic imports resolve to one native namespace and no factory is emitted.                    |
| Non-literal dynamic import remains carrier-relative        | `dynamic_expression_skipped`; a common chunk containing `import(specifier)` remains a file.                                 |
| `import.meta` keeps logical module identity                | `import_meta_skipped`; both entries observe the URL of one remaining common-chunk file.                                     |
| External import path/order remains native                  | `external_dependency_skipped`; a candidate with an external dependency stays a file.                                        |
| Non-selected dependency order remains native               | `non_inlined_dependency_skipped`; a candidate that imports a manual chunk stays a file and preserves order.                 |
| Unresolved globals keep their scope                        | `unresolved_global_capture`; copied reads stay global and registry imports receive safe local aliases.                      |
| Generated factory parameters do not capture globals        | `generated_name_unresolved_skipped`; a colliding unresolved protocol name keeps the chunk as a file.                        |
| Direct eval cannot observe injected scope                  | `direct_eval_skipped` and `eval_consumer_skipped`; eval on either side retains the native file boundary.                    |
| Manual chunk authority                                     | `manual_chunk_skipped`; the named manual chunk remains an output file.                                                      |
| Synchronous contract                                       | `top_level_await_skipped`; TLA executes normally from a remaining common file.                                              |
| Duplicated source mappings                                 | `sourcemap`; both carrier maps contain tokens and source content for `shared.js`; the snapshot visualizes both mappings.    |
| Plugin-visible physical placement                          | Node behavior test covers carrier filename callbacks, `renderChunk`, output and `generateBundle` ledgers, and exports.      |
| No ghost filename callback or reservation                  | Node behavior test proves the omitted logical chunk triggers no callback and the runtime receives the first requested name. |
| Stable and unique registry keys                            | Node tests cover content changes, distinct logical IDs, and identical projects under two checkout roots.                    |
| Escaped registry import path                               | `quoted_filename`; executes output whose runtime chunk filename contains literal quotes.                                    |
| Disabled path                                              | Node behavior test compares omitted option and `maxSize: 0` output byte-for-byte.                                           |
| Unsupported option combinations                            | Node table tests format, code splitting, strict-order opt-out, preserve modules, entry signatures, and on-demand wrapping.  |

The source-map snapshot checks that the copied body is not merely present in `sourcesContent`; its
generated tokens point into both carrier files.

### Why the current-architecture approach is conservative

Changing `module_to_chunk` to express several owners would spread through linking, tree shaking,
symbol ownership, init obligations, naming, finalization, chunk metadata, and hashing. The chosen
approach leaves those logical facts intact and records late physical placements only after
cross-chunk links are complete. This confines the policy to late generate stages and makes
unsupported shapes fall back to existing files.

The price is extra work at stage boundaries:

- `Chunk` temporarily represents a live logical unit that may not produce an asset.
- Naming must run inlined chunks before their carriers rather than fully in parallel.
- Rendering has a phase barrier because carriers need completed factories and maps.
- Runtime merging is disabled before selection knows whether any candidate survives.
- Plugin ledgers need an explicit merge because logical ownership alone no longer describes physical
  output.

These are build-time costs and structural debt, not runtime correctness shortcuts. No benchmark is
included. Large graphs can pay for reachability sets and sequential naming, so production evaluation
should measure build time, total transferred bytes, and request reduction together; `maxSize`
deliberately trades duplicated bytes for fewer requests.

### What a later refactor must preserve or improve

A later chunking refactor should preserve:

- per-chunk factories with static linking inside each factory;
- one registry record and failure state per logical key;
- strict source-order behavior and plain-function call behavior;
- live property-backed exports;
- registration-before-require proof across cyclic chunk graphs;
- stable identity keys independent of content;
- source mappings and truthful plugin-visible physical placements; and
- conservative fallback for shapes the output model cannot represent.

It should improve:

- a first-class distinction between logical chunks and physical placements;
- placement data outside the general `Chunk` ownership record;
- runtime retention decided after selection, so a build with no surviving candidates pays no
  standalone-runtime cost;
- a reusable logical-to-physical placement abstraction instead of feature-local reachability maps;
- naming that supports several placements without reserving whole canonical-name sets; and
- explicit downstream policies for CSS, declarations, preload graphs, HMR, and other consumers of
  the module-to-chunk relation.

Until those policies are implemented and tested, the option remains experimental rather than a
general replacement for automatic common chunks.
