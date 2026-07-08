# Code Splitting Design

This document records the target architecture for selective strict execution order. The current implementation is described in [implementation.md](./implementation.md). The design is intentionally stricter than the current proof of concept: order scheduling must not be represented as interop wrapping, and generate-stage lowering must be unable to reopen user-code liveness.

## Problem

`WrapKind` answers an input-module representation question. `Cjs` and `Esm` wrapping participate in linking because they determine namespace shape, binding access, `require()` behavior, and tree-shaking dependencies. Selective strict execution order answers a different output-layout question: whether a module body must be delayed because the generated chunk graph would otherwise execute it too early.

The order decision can only be made after provisional chunk placement. Reusing `WrapKind::Esm` for that late decision makes generate-stage scheduling appear to be a new interop fact. It also exposes link-owned fields such as `wrapper_ref`, `wrapper_stmt_info`, and `stmt_info_included` to late mutation. Tests can detect many incorrect mutations, but the architecture should make them impossible.

## Goals

- `LinkingMetadata::wrap_kind()` remains the immutable interop decision produced by linking.
- User module and statement liveness are fixed before order planning starts.
- Order lowering may add only synthetic wrapper, init, runtime, facade, symbol, and topology state.
- Finalization and cross-chunk linking consume interop wrappers and order wrappers through an explicit shared read interface.
- Empty order plans preserve the flag-off output path without allocating order-wrapper state.
- The fuzzer remains the semantic verifier. Rolldown exposes machine-readable facts but does not add a second runtime semantics engine or assertions that merely hide lowering bugs.

## Non-Goals

- Stronger top-level-await semantics than the default build.
- Re-running the full link stage after chunk placement.
- A conservative wrap-all fallback for graph shapes already represented by the order model.
- Changing CJS or require-of-ESM interop output when no order wrapper is selected.
- Moving general tree-shaking state out of `LinkingMetadata`; this design isolates only post-planning synthetic state.

## Rejected Alternatives

### Late `WrapKind` override

This is the current proof-of-concept bridge. It reuses mature wrapper code, but it conflates representation with scheduling and requires generate-stage code to repair link-owned metadata. Keeping the bridge would preserve the architectural problem even if every known fixture passed.

### Re-link after planning

The planner could change module representation and then repeat binding, reference propagation, tree shaking, and chunking. This would restore consistency, but it would make output generation perform a second global compiler pass, increase build cost, and risk producing a different chunk graph than the one that motivated the plan.

### Internal semantic verifier

Rolldown could independently simulate final execution and reject output when the simulation disagrees with source order. That duplicates the fuzzer oracle inside the compiler and turns a lowering bug into a build failure. The compiler should expose the final plan and event graph; the external differential oracle should judge semantics.

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
  modules: IndexVec<ModuleIdx, Option<OrderWrappedModule>>,
  runtime_helpers: RuntimeHelper,
  entry_facades: FxIndexSet<ModuleIdx>,
  synthetic_symbols: FxIndexSet<SymbolRef>,
}

pub struct OrderWrappedModule {
  pub wrapper_ref: SymbolRef,
  pub init_is_noop: bool,
  pub transitive_init_targets: FxHashMap<StmtInfoIdx, Vec<ModuleIdx>>,
}
```

`OrderWrapState` is the sole owner of these fields. Helper views may borrow it, but the data is not mirrored into `LinkingMetadata`.

- wrapper symbols and init metadata belong to order state, not `LinkingMetadata`;
- order state does not contain mutable user-statement inclusion;
- entry-facade and runtime requirements are explicit outputs of lowering;
- the table is absent for flag-off and empty-plan builds.

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

Order wrappers are emitted synthetic declarations. They do not add a `StmtInfo` that tree shaking must rediscover. Lowering creates the wrapper symbol, records its runtime helper and init dependencies, and marks the symbol as an output dependency through order state. Cross-chunk linking assigns its chunk and import/export aliases exactly as it does for other synthetic chunk symbols.

The order wrapper body contains only user statements that were already included at the finalization boundary. Excluded ordinary imports cannot gain init calls. Excluded re-exports may retain precomputed forwarding init obligations because those obligations are part of the retained export contract, not reopened statement liveness.

### Finalizer

The module finalizer has three explicit cases:

- CJS interop wrapper from `WrapKind::Cjs`;
- ESM interop wrapper from `WrapKind::Esm`;
- execution-order wrapper from `OrderWrapState`.

The execution-order case reuses the established hoisted `function init_*()` code shape but obtains its symbol and init facts from order state. It never observes an overridden interop kind.

### Topology

`OrderWrapState` drives module placement, runtime placement, CJS entry-facade splitting, restored dynamic facades, and final chunk renumbering. `finalize_chunk_plan()` remains the single boundary after which topology-derived metadata is final. Namespace and external-entry facts are recomputed only when order state changes topology.

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
  -> emit versioned trace
```

## Trace Contract

`StrictExecutionOrderPlanReady` moves to version 2. Version 2 keeps the provisional analysis and final chunk/init facts, but it stops describing order wrapping as a changed interop kind.

Each included module reports:

- immutable `interop_wrap_kind`;
- `order_wrapped`;
- final and entry chunk IDs;
- the selected wrapper origin and inclusion state when an init target exists;
- TLA taint.

The fuzzer parser accepts the versioned schema and reconstructs the final event graph from chunk imports, wrapper origins, and init obligations. Trace analysis remains diagnostic; source-versus-bundle execution remains the verdict.

## Invariants

- No generate-stage call can change `LinkingMetadata::wrap_kind()`.
- No order-lowering call can set a user statement inclusion bit.
- Every order wrapper has exactly one symbol owner and one rendered chunk.
- Every synthesized init call references a reachable interop or order wrapper.
- Every ordinary-import init obligation corresponds to a link-stage execution dependency.
- Every excluded-statement init obligation is a retained re-export obligation.
- Empty order state is observationally identical to strict execution order being disabled.

## Verification

Implementation proceeds test-first:

1. Add a structural test proving strict order leaves every module's interop `WrapKind` unchanged.
2. Add a structural test proving user statement inclusion is identical before and after lowering.
3. Port existing strict-order invariants and mixed ESM/CJS fixtures without snapshot weakening.
4. Upgrade the devtools action and fuzzer parser to version 2.
5. Run installed and local differential campaigns with traced and trace-disabled builds.
6. Run full Rust, Node, WASI, Vite, formatting, lint, and repository validation.

The migration is complete only after `override_wrap_kind()`, `hoist_esm_wrapper`, and order-specific reads of interop wrapper fields are removed.
