# Code Splitting Design

This document records the architecture for selective strict execution order. The current implementation is described in [implementation.md](./implementation.md). Order scheduling is not represented as interop wrapping, and generate-stage lowering cannot reopen user-code liveness.

## Problem

`WrapKind` answers an input-module representation question. `Cjs` and `Esm` wrapping participate in linking because they determine namespace shape, binding access, `require()` behavior, and tree-shaking dependencies. Selective strict execution order answers a different output-layout question: whether a module body must be delayed because the generated chunk graph would otherwise execute it too early.

The order decision can only be made after provisional chunk placement. Reusing `WrapKind::Esm` for that late decision makes generate-stage scheduling appear to be a new interop fact. It also exposes link-owned fields such as `wrapper_ref`, `wrapper_stmt_info`, and `stmt_info_included` to late mutation. Tests can detect many incorrect mutations, but the architecture should make them impossible.

## Goals

- `LinkingMetadata::wrap_kind()` remains the immutable interop decision produced by linking.
- User module and statement liveness are fixed before order planning starts.
- Order lowering may add only synthetic wrapper, init, runtime, facade, symbol, and topology state.
- Finalization and cross-chunk linking consume interop wrappers and order wrappers through an explicit shared read interface.
- Flag-off builds do not allocate order-wrapper state or create strict-only facades.
- The external differential fuzzer remains the semantic verifier. Rolldown does not add a test-only execution model or assertions that merely turn lowering bugs into build failures.

## Non-Goals

- Stronger top-level-await semantics than the default build.
- Re-running the full link stage after chunk placement.
- A conservative wrap-all fallback for graph shapes already represented by the order model.
- Changing CJS or require-of-ESM interop output when no order wrapper is selected.
- Moving general tree-shaking state out of `LinkingMetadata`; this design isolates only post-planning synthetic state.

## Rejected Alternatives

### Late `WrapKind` override

This was the original proof-of-concept bridge. It reused mature wrapper code, but it conflated representation with scheduling and required generate-stage code to repair link-owned metadata. Keeping the bridge would preserve the architectural problem even if every known fixture passed.

### Re-link after planning

The planner could change module representation and then repeat binding, reference propagation, tree shaking, and chunking. This would restore consistency, but it would make output generation perform a second global compiler pass, increase build cost, and risk producing a different chunk graph than the one that motivated the plan.

### Internal semantic verifier

Rolldown could independently simulate final execution and reject output when the simulation disagrees with source order. That duplicates the fuzzer oracle inside the compiler and turns a lowering bug into a build failure. The external differential oracle should judge the normal generated output instead.

## Target Architecture

### Immutable link state

`LinkingMetadata` owns only link facts:

- interop `wrap_kind`, `wrapper_ref`, and `wrapper_stmt_info`;
- user statement and module inclusion;
- linked exports, namespace decisions, and execution dependencies;
- TLA and interop metadata.

`override_wrap_kind()` and `hoist_esm_wrapper` are removed. Generate-stage order code receives no API that can change interop kind or user inclusion.

### `OrderWrapState`

Generate-stage finalization creates an optional side table:

```rust
pub struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, OrderWrappedModule>,
  synthetic_statements: Vec<OrderSyntheticStmt>,
  import_overlays: FxHashMap<OrderImportKey, OrderImportOverlay>,
  runtime_helpers: RuntimeHelper,
  entry_facades: FxIndexSet<ModuleIdx>,
  namespace_requirements: FxIndexSet<SymbolRef>,
}

pub struct OrderWrappedModule {
  pub wrapper_ref: SymbolRef,
  pub wrapper_statement: OrderSyntheticStmtIdx,
  pub init_is_noop: bool,
  pub transitive_init_targets: FxHashMap<StmtInfoIdx, Vec<ModuleIdx>>,
}

pub struct OrderSyntheticStmt {
  pub owner: ModuleIdx,
  pub declared_symbols: Vec<TaggedSymbolRef>,
  pub referenced_symbols: Vec<SymbolRef>,
  pub runtime_helpers: RuntimeHelper,
  pub chunk: Option<ChunkIdx>,
}

pub struct OrderImportKey {
  pub importer: ModuleIdx,
  pub statement: StmtInfoIdx,
  pub record: ImportRecordIdx,
}

pub struct OrderImportOverlay {
  pub referenced_symbols: Vec<SymbolRef>,
  pub runtime_helpers: RuntimeHelper,
  pub requires_importer_namespace: bool,
  pub requires_importee_namespace: bool,
  pub reexports_dynamic_exports: bool,
}
```

`OrderWrapState` is the sole owner of these fields. Helper views may borrow it, but the data is not mirrored into `LinkingMetadata`.

- wrapper symbols and init metadata belong to order state, not `LinkingMetadata`;
- order state does not contain mutable user-statement inclusion;
- importer-specific references and runtime helpers belong to `import_overlays`, not the original `StmtInfo`;
- synthetic declarations participate in chunk assignment and deconfliction through an explicit synthetic-statement API;
- entry-facade and runtime requirements are explicit outputs of lowering;
- namespace requirements are recorded independently of whether topology changed;
- the table stays empty when no wrappers or import overlays are needed.

### Lowering API boundary

The lowerer receives link data through immutable references. Its mutable output surface contains only the symbol database, chunk graph, and the new order state:

```rust
pub struct OrderLoweringInput<'a> {
  pub plan: &'a OrderWrapPlan,
  pub modules: &'a IndexModules,
  pub linking: &'a LinkingMetadataVec,
  pub statements: &'a IndexVec<ModuleIdx, StmtInfos>,
}

pub struct OrderLoweringOutput<'a> {
  pub symbols: &'a mut SymbolRefDb,
  pub chunks: &'a mut ChunkGraph,
  pub state: &'a mut OrderWrapState,
}
```

The API does not expose mutable `LinkingMetadata` or `StmtInfos`. Topology-derived link facts are recomputed by separate finalization passes after lowering; the lowerer communicates new namespace and entry requirements through `OrderWrapState`.

### Shared init-target view

Finalization and cross-chunk linking need to work with two sources of lazy initialization:

1. interop ESM wrappers from `LinkingMetadata`;
2. order wrappers from `OrderWrapState`.

They use a read-only view instead of testing an effective `WrapKind`:

```rust
pub struct EsmInitTarget {
  pub wrapper_ref: SymbolRef,
  pub init_is_noop: bool,
  pub tla_tainted: bool,
  pub origin: EsmInitOrigin,
}

pub enum EsmInitOrigin {
  Interop,
  ExecutionOrder,
}
```

An accessor resolves at most one ESM init target for a module. Interop ESM wrapping takes precedence because an already interop-wrapped module is represented by that existing wrapper; the order planner selects an eligible carrier instead of adding a second wrapper.

### Synthetic symbol inclusion

Order wrappers are emitted synthetic declarations. They do not add a user `StmtInfo` that tree shaking must rediscover. Lowering creates an `OrderSyntheticStmt`, which is live by construction and provides the declared symbols, referenced symbols, runtime helpers, and eventual chunk assignment that cross-chunk linking and deconfliction require.

`used_symbol_refs` remains sealed after lowering, but cross-chunk liveness uses a composite view: link-stage used symbols plus every symbol declared or referenced by a live `OrderSyntheticStmt` or `OrderImportOverlay`. Symbol-to-chunk assignment and root-scope deconfliction explicitly iterate synthetic statements instead of discovering them through link-stage statement tables.

The order wrapper body contains only user statements that were already included at the finalization boundary. An excluded ordinary import may retain a synthetic init obligation only when link-stage `execution_dependencies` already records that its target must execute. Excluded re-exports may retain forwarding init obligations because those obligations are part of the retained export contract. Neither case marks the original user statement as included.

### Import overlay

Changing an importee from eager execution to an order wrapper affects its importers even though their user statements do not become live. The overlay records the synthetic consequences currently repaired by mutating `StmtInfo`:

- wrapper and namespace symbol references;
- `ReExport` and `ToCommonJs` runtime helpers;
- dynamic-export re-export behavior;
- importer and importee namespace requirements;
- direct and transitive init obligations.

Finalization and cross-chunk linking read the overlay alongside the immutable original import record. Tree shaking and user statement inclusion never read it.

### Finalizer

The module finalizer has three explicit cases:

- CJS interop wrapper from `WrapKind::Cjs`;
- ESM interop wrapper from `WrapKind::Esm`;
- execution-order wrapper from `OrderWrapState`.

The execution-order case reuses the established hoisted `function init_*()` code shape but obtains its symbol and init facts from order state. It never observes an overridden interop kind.

Removed user import/re-export statements are finalized with any matching `OrderImportOverlay`. The finalizer may emit a synthetic init or re-export expression in the removed statement's source position, but it does not restore the original statement.

### Entry prologue

Entry rendering consumes the same init-target view as module finalization. Order-wrapped entries emit an explicit init call. Interop entries used internally also keep an inert implementation chunk behind their public facade.

### Topology

`OrderWrapState` drives module and runtime placement. Strict entry facades can also change topology without an order wrapper. `finalize_chunk_plan()` remains the boundary after which topology-derived metadata is final.

## Data Flow

```text
link + tree shaking
  -> immutable LinkingMetadata and execution dependencies
  -> provisional ChunkGraph
  -> OrderAnalysis / OrderWrapPlan
  -> lower plan into OrderWrapState + final ChunkGraph
  -> compute init metadata using LinkingMetadata + OrderWrapState
  -> compute cross-chunk links using the shared EsmInitTarget view
  -> finalize modules using explicit interop/order wrapper cases
  -> render entry prologues using the shared EsmInitTarget view
```

## Invariants

- No generate-stage call can change `LinkingMetadata::wrap_kind()`.
- No order-lowering call can set a user statement inclusion bit.
- Every order wrapper has exactly one symbol owner and one rendered chunk.
- Every synthetic declaration participates in symbol-to-chunk assignment and deconfliction.
- Every import overlay is backed by an immutable link-stage execution dependency or retained re-export contract.
- Every synthesized init call references a reachable interop or order wrapper.
- A planned static chunk SCC includes every eligible order-sensitive module in that SCC.
- Every ordinary-import init obligation corresponds to a link-stage execution dependency.
- Every excluded-statement init obligation is either a retained re-export obligation or a synthetic obligation backed by an execution dependency.
- Every order-wrapped entry has an explicit entry trigger.
- Flag-off builds create no order wrappers or strict-only entry facades.

## Verification

Implementation proceeds test-first:

1. Add a structural test proving strict order leaves every module's interop `WrapKind` unchanged.
2. Add a structural test proving user statement inclusion is identical before and after lowering.
3. Add focused tests for importer overlays, synthetic symbol assignment, namespace requirements, and entry prologues.
4. Port existing strict-order invariants and mixed ESM/CJS fixtures without snapshot weakening.
5. Run installed and local black-box differential campaigns against normal generated output.
6. Run full Rust, Node, WASI, Vite, formatting, lint, and repository validation.

The migration is complete only after `override_wrap_kind()`, `hoist_esm_wrapper`, and order-specific reads of interop wrapper fields are removed.
