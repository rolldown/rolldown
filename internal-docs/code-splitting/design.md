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
- Flag-off builds leave order-wrapper state empty and create no strict-only facades.
- The external differential fuzzer remains the semantic verifier. Rolldown does not add a test-only execution model or assertions that merely turn lowering bugs into build failures.

## Modes

`strictExecutionOrder: true` alone runs **wrap-all**: every eligible module defers, the eager
phase contains only inert definitions, and no evaluation-order prediction is needed —
correctness rests solely on the shared lowering and trigger placement. It is the default
because its trust base is the smaller one, and it serves as the escape hatch when the
selective analysis misjudges a shape.

`experimental.onDemandWrapping: true` opts into the **on-demand** analysis described below,
which starts from predicted evaluation-order hazards and conservatively closes the plan over
cases where safety requires additional wrappers. Both modes share the plan/lowering/consumer
pipeline; they differ only in how the plan is seeded.

This difference is one-way: wrap-all may create more inert wrappers, but it must not retain or
execute more user code. Link-stage statement and binding liveness is final before either plan is
lowered, so wrap-all and on-demand preserve the same tree-shaking result.

## Conservative decisions

These are the main places where strict output deliberately accepts extra wrappers for certainty:

- **Wrap-all mode** wraps everything eligible (see Modes).
- **Chunk-cycle bailout** (on-demand): a root that can reach a static chunk cycle over the
  predicted edges additionally wraps every eligible module in its expected order. Within a
  cycle, evaluation order depends on the chunk the runtime enters first, and lowering itself
  moves that entry point; the prediction also cannot see `var`-form interop wrapper
  definitions that another cycle chunk calls eagerly.
- **Entry-trigger facades**: an inline entry trigger fires whenever its chunk is evaluated,
  so an entry whose chunk anything else _can_ load — order-wrapped or interop-wrapped, in both
  strict modes — moves its trigger to a facade. The question is answered by running the real
  cross-chunk link computation against the fully lowered order state
  (`lowered_static_import_edges`), not a prediction, so an entry whose chunk only it can load
  keeps its trigger inline and costs no extra file. "Loads" covers both static imports from
  other chunks and cross-chunk dynamic imports of any other module hosted in the entry chunk —
  e.g. a manual group placing a dynamic target next to an entry, where the `import()` evaluates
  the entry chunk. A dynamic import of the entry module itself must run its program, and a
  dead dynamic import lowers to an inert stub, so neither forces the split; any other surviving
  record counts as a possible load even if never executed.
  A pure dynamic entry is the exception when every live `import()` call site can carry the trigger:
  the implementation chunk becomes a common chunk and each call site rewrites to either
  `Promise.resolve().then(() => (init_*()..., namespace))` or
  `import(host).then(n => (n.init_*()..., n.namespace))`. The activation is normally one module
  wrapper; for a consumer-local namespace it is the route's complete leaf/CJS-carrier target list,
  never the intentionally empty shared barrel wrapper. This applies both to facades removed by the
  chunk optimizer and to facades that strict lowering would otherwise create. It is rejected for a
  previously restored empty facade, an emitted/user entry, a TLA-tainted target, a cross-chunk host
  namespace that may expose callable `then`, or — on the create path — a direct or transitive
  export-star chain that reaches an external module. Entry-level external merges render on the
  facade chunk in every format, and the module-local simulated namespace does not reproduce their
  format-specific behavior. The external-star guard is create-path-only: the restore path instead
  relies on the chunk optimizer's simulated-namespace handling, whose external-star preservation is
  handled separately.
- **CJS namespace merge is skipped under strict** (`determine_safely_merge_cjs_ns`): merging
  moves the surviving require call to whichever statement stays included — an intra-body
  move no wrapping can repair. Per-importer call sites cost bytes; the wrapper memoizes.
- **`expected ∖ actual` seeds**: an order-sensitive module invisible to the predicted order
  (tree-shaking considers it side-effect-free) is wrapped rather than trusted.

## Trigger placement

Every site that can run a wrapped module, in one place:

| Trigger                                                                           | Lives in                                                                                 | Owner                                                                                              |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `init_*()` for an order-wrapped importee of a live statement                      | importer body, statement position                                                        | finalizer via the shared init-target view                                                          |
| `init_*()` / `require_*()` obligations of removed statements                      | importer body, removed statement's position                                              | `OrderImportOverlay` / transitive init targets                                                     |
| generated CJS re-export interop from a consumer-local barrel                      | importer body, routed statement position; declaration beside the CJS importee            | per-import-record `OrderCjsCarrier`                                                                |
| user or dynamic entry activation, unless every live `import()` rewrite carries it | entry chunk prologue (a facade only when other chunks can load the implementation chunk) | `create_order_wrap_entry_facades` / `restore_order_wrap_entry_facades`                             |
| collapsible dynamic entry activation                                              | importer body, the rewritten `import()` call site                                        | finalizer `rewrite_dynamic_import_for_merged_entry` via the shared module-or-namespace target view |
| interop `require_*()` of an eager importer                                        | importer body (its carrier)                                                              | flag-off interop machinery, order-analysis carrier rule                                            |

A trigger must never sit inline in a chunk body that other chunks can evaluate as a
dependency; that is the facade rule's content.

Moving a dynamic entry trigger to the promise continuation deliberately puts it one microtask
after the host chunk settles. The importing promise still resolves only after `init_*()` runs, but
a microtask queued during host evaluation can observe the target before initialization. Both strict
modes use this policy; `m4_dynamic_facade_race` pins it.

## Thenable chunk namespaces

`import()` resolves through the promise resolution procedure, so a chunk namespace that carries a
callable `then` export is assimilated: the promise settles with whatever that `then` produces, and
the call-site rewrite's extraction callback never receives the namespace. For a merged dynamic
entry this would let a chunk-mate's export change what importing the target observes — impossible
in source, where importing a module never exposes a sibling's exports.

The defense splits by who owns the export name:

- **Bundler-owned names are never `then`.** `deconflict_exported_names` reserves it before naming
  internal exports — a source symbol named `then` deconflicts to `then$1` like any collision — and
  the minified-name generator skips the literal `then` (it would otherwise appear at value
  443,179). An `emitFile`-promised export keeps `then` only when `then` is the promised name
  itself; #10500 tracks routing those through the predefined-names path, which removes that
  carve-out.
- **User-observable names cannot be renamed, so the collapse is refused instead**: an entry's own
  public exports, `emitFile`-promised names, and whatever an `export * from` chain reaching an
  external module supplies at runtime. `order_wrap_host_can_expose_then_export` guards the restore
  path and the entry-facade decision guards the create path — see the entry-trigger facade bullet
  above. In addition to that, `dynamic_entry_supports_namespace_extraction`, which rewrites dynamic
  importers to `.then((n) => n.<ns>)`, will avoid inlining the module into the dynamic entry.
  Note that this is only needed when the exports used by the dynamic import are not a known subset
  of the ones exported by the dynamic entry module. If they are a known subset, then we do not need
  to generate this intermediate namespace and we can directly inline the module even though there
  is a `then` export of `export * from` (see `dynamic_entry_partial_usage_allows_plain_merge`).

Deliberately kept: a dynamic-import target's own `then` export. Native `import()` of the source
module assimilates the same way, so renaming it would diverge from source semantics rather than
preserve them.

## Tree-shaking parity across strict modes

Late order lowering cannot make an excluded import binding or re-export live. The difficult case
is a pure re-export barrel selected by wrap-all: its wrapper exists, but making that shared wrapper
own every downstream `init_*` would initialize leaves used by unrelated consumers. Such a wrapper
is marked re-export-transparent when it has no local executable body, generated missing-export
assignment, unconditional execution dependency, or `keepNames` work. Each consumer then routes
through it only to the leaf bindings that consumer retained.

A direct CJS re-export used to disqualify an otherwise pure barrel because the ordinary CJS
finalizer generated `namespace = __toESM(require_cjs())` inside the shared barrel wrapper. Strict
lowering now represents that generated body as one `OrderCjsCarrier` per import record. The barrel
wrapper becomes an empty routing waypoint; a consumer of `cn` reaches only `cn`, while a consumer of
`cloneDeep` additionally calls the carrier for that exact CJS record. Two CJS re-export records
never share a monolithic init, even when they target the same CJS module. A carrier declaration is
placed beside its CJS importee, is inert until its memoized init is called, and owns the original
record's namespace conversion and Node-interop mode.

An effectful CJS re-export remains an eager obligation of every evaluation of that barrel route
only when the barrel's own side-effect contract retains that record unconditionally. A
`moduleSideEffects: false` barrel may retain a CJS record globally for one lazy binding consumer
without making that record eager for unrelated consumers. The same boundary applies at every hop
through nested consumer-local barrels: an outer `moduleSideEffects: false` barrel blocks a deeper
carrier from becoming an unconditional obligation even when each inner barrel retains side
effects. The resolver recursively collects only the carriers whose complete forwarding path
retains side effects, then sorts them with selected leaves by their complete source-record path,
preserving `effect-before`, leaf, `effect-after` order. A bare import has no binding route but still
receives the genuinely eager carriers. Retained-path traversal carries the same permission through
every selected hop: a carrier explicitly selected by binding demand still crosses a pure boundary,
but an off-path eager carrier does not.

The routing evidence is consumer-local. Named imports use their local facade's link-stage liveness,
including facades retained through an export chain. Namespace holders — both `import * as ns` and a
named import whose value is a namespace — inspect only included statements: statically resolved
member reads route that member, while opaque uses expand the non-ambiguous namespace. A resolved
member that the constant-inlining pass will replace is skipped using the same constant metadata and
inline mode as tree shaking. Module-global leaf or namespace liveness is deliberately insufficient,
because another importer can make the same canonical symbol live without retaining it for this
consumer.

The optimization is deliberately restricted to source modules whose body consists entirely of
direct re-exports. Tree shaking must be enabled, and the module must not be in a synchronous
`import`/`require` SCC, be the target of an opaque CommonJS `require()`, carry top-level await or a
TLA dependency, expose dynamic exports, require a missing-export shim, or be a concatenated
wrapper. CJS `export *` is also excluded because its namespace is dynamic. These shapes keep the
existing monolithic initialization path. Rejecting a synchronous SCC is the first-line cycle
safety rule: it preserves one module's evaluating guard across dependency traversal and local body
instead of flattening a cycle into separately ordered leaf calls. A direct `require()` target also
stays monolithic. When that monolithic wrapper (or any other non-routing wrapper) retains an
`export *` into a consumer-local route and materializes the resulting namespace, its guarded
record position initializes the route's complete leaf/carrier target list before exposing the
namespace getters.

Code-splitting placement makes the same consumer-local decision before chunks exist. A probe
`OrderWrapState` is built from the wrap-all structural plan, and entry reachability routes an
incoming record through the shared resolver. Once a non-entry consumer-local barrel is reached,
its module-wide `load_dependencies` union is not traversed; only recursively eager carriers and
consumer-local waypoint modules are unconditional. Named leaves and pure carriers get bits only
from the incoming consumer that chose them. Preserving waypoint modules keeps every included
routing barrel reachable without reopening its union of unrelated leaves. A barrel that is itself
a user or dynamic entry exposes its complete namespace and therefore uses the conservative full
traversal and an entry prologue containing every statically known target.

A namespace synthesized only to replace a collapsed dynamic-entry facade is not an opaque namespace
consumer. Its getters are restricted to the export interface already retained by link-time
`import()` consumers, and its synthetic statement references every non-inlined binding behind those
getters so cross-chunk linking cannot leave a dangling getter after the entry chunk becomes common.
The restriction is by export name, not merely canonical symbol: another consumer retaining the same
binding under a different alias must not widen this dynamic-entry interface. If the module namespace
also has a real semantic consumer, that complete semantic interface wins. Excluded re-export init
routing otherwise continues to use the dynamic consumers' recorded paths. Treating the synthetic
namespace as opaque would discard those paths and can skip a required leaf initializer in a
re-export cycle. `cross_chunk_dynamic_importer_uses_call_site_trigger` pins the narrowed simulated
facade independently of the retained-star fixtures' routing topology.

## Audit decisions

Two shapes were challenged and deliberately kept:

- **The prediction runs the real cross-chunk link pass twice** (on-demand only). An
  edges-only fork and a cached-state reuse were both designed and rejected: the init
  metadata pass writes between prediction and the final link run, so any shortcut drifts
  from emission — the very failure the prediction exists to prevent. The double run is the
  fidelity mechanism; wrap-all mode skips prediction entirely.
- **Interop-wrapped modules appear inside the expected/actual orders** rather than being
  collapsed to carrier attributions. An attribution-identity model was implemented and
  reverted: two different carriers can fire a trigger at the same sequence position (host
  identity differs, order does not), so identity comparison over-wraps. The in-order
  representation is the sequence semantics; trigger-host transfer is the targeted
  repair for at-risk modules that cannot themselves be delayed.

## Emergent-cycle plan projection (on-demand)

Two distinct prediction mechanisms exist; keep them separate.

1. **Initial full-link prediction** (`predicted_static_import_edges`) runs the real cross-chunk
   link pass with an _empty_ order state to obtain the pre-lowering baseline chunk topology (value
   and side-effect edges, no `init_*` wrapper imports). This is the "runs the link pass twice"
   fidelity mechanism above; the plan and the at-risk analysis are computed against it.

2. **Iterative plan projector** (`post_lowering_import_edges`) closes the loop that a one-shot
   analysis misses: applying a wrap plan makes the lowering add its own cross-chunk `init_*`
   forwarding imports, which can close chunk cycles the acyclic baseline never showed. An eager
   module hosted in such an emergent cycle runs its record-position `init_*()`/`require_*()` during
   the cycle before a sibling chunk assigned its wrapper var — the `require_* is not a function`
   startup crash. So each round the projector layers the plan's forwarding edges on the baseline,
   finds the chunk SCCs they close, marks every eligible module in a cyclic chunk at-risk, and
   rebuilds the plan until the at-risk set stops growing (monotone and finite — the projected
   _topology_ is not monotone, edges can shrink when a newly wrapped forwarder stops a deeper walk).
   `ROLLDOWN_ORDER_DEBUG=1` traces per-round SCC counts and the final wrap delta.

The projector reproduces, from a discovery-only probe order state (the same wrappers, nested-record
set, and per-record overlays the real lowering mints), exactly the three `init_*` dependency kinds
the linker registers — so it stays in lockstep with emission instead of forking a shortcut:

- **Retained re-export overlays** — an importer's `OrderImportOverlay` referencing an order-wrapped
  target's wrapper, registered with no init-owner gate, so an _eager_ forwarder's cross-chunk hop
  counts too. Admitted only for order-wrapped (not interop `WrapKind::Esm`) non-nested targets.
- **Included + retained excluded re-export forwarding** — a wrapped importer's included imports and
  retained excluded re-export hops, via the shared `collect_wrapped_esm_init_targets_for_import_record`.
- **Non-included forwarder hops** — a wrapped importer's re-export of a _non-included_ forwarder,
  walking the forwarder's every static import (not just its resolved exports), the excluded-statement
  metadata routing.

Deliberately omitted, and why it is sound: interop wrapper edges (already in the baseline) and
nested/consumption-gated hops (owned by a wrapped ancestor, tree-shaken for an eager forwarder) are
skipped so the projection never over-wraps a tree-shaking-equivalent graph; entry-facade edges are
omitted because a facade holds zero modules, hence zero static indegree, and can never sit in a
static SCC (debug-asserted in the final link pass). The remaining over-approximation (it omits the
wrapping's own liveness suppression) only ever wraps more, which is always legal — wrap-all is the
standing proof.

## Non-Goals

- Stronger top-level-await semantics than the default build.
- Re-running the full link stage after chunk placement.
- A conservative wrap-all fallback for graph shapes already represented by the order model.
- Changing CJS or require-of-ESM interop output when no order wrapper is selected.
- Moving general tree-shaking state out of `LinkingMetadata`; this design isolates only post-planning synthetic state.

### Contract boundaries: no ordering promise, but always valid output

Two input classes are outside the ordering promise, yet the emitted code must stay valid and executable:

- **Top-level await.** Order wrapping makes no TLA promise beyond the default build. Mechanically it stays valid: a TLA-tainted module (or one that transitively depends on one) gets an `async` wrapper body, and every emitted `init_*()` call site awaits when the target is tainted (`EsmInitTarget::tla_tainted`), so the taint propagates with the wrappers and `await` never lands in a sync function.
- **External modules.** A static ESM `import` of an external cannot be deferred without changing semantics (it hoists to the top of its chunk and evaluates at chunk load), so an external's side effects can run earlier than source order when its importer is wrapped — for static ESM output this is unfixable by wrapping and matches every other bundler. Emitted code stays valid: external import statements survive at chunk top and wrapped importers reference their bindings from inside closures.

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

Generate-stage finalization creates a side table that remains empty unless strict lowering records order-owned state:

```rust
pub struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, OrderWrappedModule>,
  reexport_init_transparent: FxHashSet<ModuleIdx>,
  consumer_local_reexport_routes: FxHashSet<ModuleIdx>,
  consumer_local_namespace_targets: FxHashMap<ModuleIdx, Vec<WrappedEsmInitTarget>>,
  cjs_carriers: FxHashMap<OrderCjsCarrierKey, OrderCjsCarrier>,
  cjs_carriers_by_importee: FxHashMap<ModuleIdx, Vec<OrderCjsCarrierKey>>,
  cjs_carriers_by_symbol: FxHashMap<SymbolRef, Vec<OrderCjsCarrierKey>>,
  cjs_carrier_by_namespace: FxHashMap<SymbolRef, OrderCjsCarrierKey>,
  cjs_carrier_wrapper_refs: FxHashSet<SymbolRef>,
  synthetic_statements: IndexVec<OrderSyntheticStmtIdx, OrderSyntheticStmt>,
  synthetic_statements_by_chunk: FxHashMap<ChunkIdx, Vec<OrderSyntheticStmtIdx>>,
  import_overlays: FxHashMap<OrderImportKey, OrderImportOverlay>,
  import_overlays_by_importer: FxHashMap<ModuleIdx, Vec<OrderImportKey>>,
  import_overlays_by_statement: FxHashMap<(ModuleIdx, StmtInfoIdx), Vec<OrderImportKey>>,
  namespace_requirements: FxHashMap<SymbolRef, FxIndexSet<ModuleIdx>>,
  runtime_symbols: FxHashSet<SymbolRef>,
  nested_reexport_records: FxHashSet<(ModuleIdx, ImportRecordIdx)>,
  consumed_reexport_facades: FxHashSet<SymbolRef>,
}

pub struct OrderWrappedModule {
  pub wrapper_ref: SymbolRef,
  pub wrapper_statement: Option<OrderSyntheticStmtIdx>,
  pub chunk: Option<ChunkIdx>,
}

pub struct OrderCjsCarrierKey {
  pub importer: ModuleIdx,
  pub record: ImportRecordIdx,
}

pub struct OrderCjsCarrier {
  pub importee: ModuleIdx,
  pub wrapper_ref: SymbolRef,
  pub namespace_ref: SymbolRef,
  pub wrapper_statement: Option<OrderSyntheticStmtIdx>,
  pub chunk: Option<ChunkIdx>,
  pub eager: bool,
  pub needs_to_esm: bool,
  pub is_node_mode: bool,
}

pub struct OrderSyntheticStmt {
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
  pub retained_reexport_path: Vec<(ModuleIdx, ImportRecordIdx)>,
}
```

`OrderWrapState` is the sole owner of these order-lowering fields. Helper views may borrow it, but the data is not mirrored into `LinkingMetadata`.

- order-wrapper symbols and placement belong to order state, not `LinkingMetadata`;
- per-import-record CJS carriers, their namespace symbols, and their importee-chunk placement belong to order state;
- order state does not contain mutable user-statement inclusion;
- importer-specific references and runtime helpers belong to `import_overlays`, not the original `StmtInfo`;
- synthetic declarations participate in chunk assignment and deconfliction through an explicit synthetic-statement API, with secondary indexes for chunk rendering; declarations need not share their symbol owner's module chunk because CJS carrier symbols are deliberately placed beside the CJS importee;
- entry facades are explicit chunk-graph changes made by the caller after lowering, while required runtime symbols are derived from synthetic statements and import overlays;
- namespace requirements retain the live importer modules that require each namespace, so a dead overlay cannot keep a namespace alive;
- nested re-export records and consumed facades preserve the frozen tree-shaking decisions used by re-export init routing;
- the table stays empty when no wrappers or import overlays are needed.

### Lowering API boundary

The lowerer receives link data through immutable references. Its mutable output surface contains only the symbol database and the new order state:

```rust
pub struct OrderLoweringInput<'a> {
  pub plan: &'a OrderWrapPlan,
  pub modules: &'a IndexModules,
  pub linking: &'a LinkingMetadataVec,
  pub statements: &'a IndexVec<ModuleIdx, StmtInfos>,
  pub asts: &'a IndexEcmaAst,
  pub keep_names: bool,
  pub export_chains: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub star_reexport_records_by_imported_symbol:
    &'a FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, ImportRecordIdx)>>>,
  pub used_symbols: &'a UsedSymbolRefsBuilder,
  pub cyclic_modules: &'a FxHashSet<ModuleIdx>,
  pub tree_shaking: bool,
}

pub struct OrderLoweringOutput<'a> {
  pub symbols: &'a mut SymbolRefDb,
  pub state: &'a mut OrderWrapState,
}
```

The API does not expose mutable `LinkingMetadata`, `StmtInfos`, or the chunk graph. The surrounding generate-stage pass places the synthetic wrappers, creates any entry facades, and recomputes topology-derived facts after lowering; the lowerer communicates new symbol, namespace, runtime, and re-export-routing requirements through `OrderWrapState`.

### Final ESM init metadata

After wrapper selection and final chunk topology are fixed, `compute_wrapped_esm_init_metadata` derives the two facts that depend on both link and order state: whether an `init_*()` call is a no-op, and which wrapped modules must be initialized at each excluded statement. Interop and execution-order wrappers share one sealed result instead of writing the same kind of final fact back into either owner's mutable state:

```rust
pub struct Sealed<T>(T); // private field and constructor; Deref only, never DerefMut or unwrap

pub struct FinalEsmInitMetadata {
  modules: FxHashMap<ModuleIdx, ModuleEsmInitMetadata>,
}

struct ModuleEsmInitMetadata {
  init_is_noop: bool,
  transitive_init_targets: FxHashMap<StmtInfoIdx, Vec<WrappedEsmInitTarget>>,
}

fn compute_wrapped_esm_init_metadata(/* ... */) -> Sealed<FinalEsmInitMetadata>;
```

`Sealed<T>` and its private constructor live in the leaf module that computes this artifact. The result cannot be unwrapped or mutably dereferenced, so taking ownership again does not reopen mutation. Final cross-chunk linking and module finalization accept only `&Sealed<FinalEsmInitMetadata>`; an unsealed value cannot satisfy either signature.

The table is sparse: a missing entry means `init_is_noop == false` and no excluded-statement targets, never that the module lacks a wrapper. Wrapper identity remains in `LinkingMetadata` or `OrderWrapState`. The earlier on-demand projection continues to recompute a conservative draft from its probe `OrderWrapState` because final metadata does not exist yet; it marks final metadata unavailable instead of manufacturing an empty sealed value.

### Shared init-target view

Finalization and cross-chunk linking need to work with three sources of lazy initialization:

1. interop ESM wrappers from `LinkingMetadata`;
2. order wrappers from `OrderWrapState`;
3. generated per-record CJS carriers from `OrderWrapState`.

They use a read-only view instead of testing an effective `WrapKind`:

```rust
pub struct EsmInitTarget {
  pub wrapper_ref: SymbolRef,
  pub tla_tainted: bool,
  pub origin: EsmInitOrigin,
}

pub enum EsmInitOrigin {
  Interop,
  ExecutionOrder,
}

pub enum WrappedEsmInitTarget {
  Module(ModuleIdx),
  CjsCarrier(OrderCjsCarrierKey),
}
```

An accessor resolves at most one ESM init target for a module. Interop ESM wrapping takes precedence because an already interop-wrapped module is represented by that existing wrapper; the order planner selects an eligible carrier instead of adding a second wrapper. Record routing returns `WrappedEsmInitTarget`, so Emit, Register, Project, and pre-chunk placement agree on whether an obligation names a module wrapper or a CJS carrier. The module view carries structural wrapper identity only; final no-op and excluded-statement facts come from `FinalEsmInitMetadata`.

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
For a non-empty retained path, the overlay keeps only namespace/runtime glue and does not separately
reference the direct wrapper; the shared resolved target list is the sole source for both emitted
calls and cross-chunk registration.

### Finalizer

The module finalizer has three explicit cases:

- CJS interop wrapper from `WrapKind::Cjs`;
- ESM interop wrapper from `WrapKind::Esm`;
- execution-order wrapper from `OrderWrapState`.

The execution-order case reuses the established hoisted `function init_*()` code shape, obtains its wrapper symbol from order state, and obtains final derived init facts from `FinalEsmInitMetadata`. It never observes an overridden interop kind.

Removed user import/re-export statements are finalized with any matching `OrderImportOverlay`. The finalizer may emit a synthetic init or re-export expression in the removed statement's source position, but it does not restore the original statement.

### Complete consumer-local namespaces and entry prologues

Lowering precomputes the complete, source-ordered namespace target list for every consumer-local route. Included import/re-export records route through the importer-local resolver even when the route also has a link-stage interop wrapper; excluded `export *` records whose namespace is materialized use the cached complete list from final metadata. Both paths keep the calls inside the consuming monolithic wrapper's evaluating guard and at the re-export record's source position. Entry rendering and collapsed dynamic-entry call-site activation consume the same list: a consumer-local barrel entry cannot call its intentionally empty shared wrapper, so its prologue or rewritten `import()` calls every namespace target instead. For a cross-chunk rewrite, the host chunk imports and re-exports every target wrapper/carrier beside the simulated namespace; the callback invokes them in list order before returning that namespace. Other order-wrapped entries emit an explicit init call, and interop entries used internally keep an inert implementation chunk behind their public facade.

### Topology

`OrderWrapState` drives module and runtime placement. Strict entry facades can also change topology without an order wrapper. `finalize_chunk_plan()` remains the boundary after which topology-derived metadata is final.

## Data Flow

```text
link + tree shaking
  -> immutable LinkingMetadata and execution dependencies
  -> pre-chunk consumer-local probe + importer-local entry-bit propagation
  -> provisional ChunkGraph
  -> OrderAnalysis / OrderWrapPlan
  -> lower plan into OrderWrapState + final ChunkGraph
  -> compute Sealed<FinalEsmInitMetadata> using LinkingMetadata + OrderWrapState
  -> compute cross-chunk links using EsmInitTarget + Sealed<FinalEsmInitMetadata>
  -> finalize modules using explicit interop/order wrapper cases + Sealed<FinalEsmInitMetadata>
  -> render entry prologues using the shared EsmInitTarget view
```

## Invariants

- No generate-stage call can change `LinkingMetadata::wrap_kind()`.
- No order-lowering call can set a user statement inclusion bit.
- Every order wrapper's declared `SymbolRef` has exactly one module owner, and its synthetic declaration has exactly one rendered chunk.
- Every CJS re-export carrier is keyed by one importer record and rendered in its CJS importee's chunk.
- Every synthetic declaration participates in symbol-to-chunk assignment and deconfliction.
- Every import overlay is backed by an immutable link-stage execution dependency or retained re-export contract.
- Every synthesized init call references a reachable interop or order wrapper.
- A planned static chunk SCC includes every eligible order-sensitive module in that SCC.
- Every ordinary-import init obligation corresponds to a link-stage execution dependency.
- Every excluded-statement init obligation is either a retained re-export obligation or a synthetic obligation backed by an execution dependency.
- Final cross-chunk registration and finalizer emission require the same `Sealed<FinalEsmInitMetadata>` type; pre-final projection cannot supply one and does not consume final metadata.
- Wrap-all and on-demand preserve the same link-stage statement and binding liveness; only their wrapper plans may differ.
- Emit, Register, Project, and pre-chunk placement resolve consumer-local records through the same target model.
- Every order-wrapped entry has an explicit entry trigger.
- Flag-off builds create no order wrappers or strict-only entry facades.

## Verification

Verification stays at observable compiler boundaries rather than reaching into private pass state:

1. Real-`Bundler` integration invariants cover flag-off legacy output, byte-identical hazard-free on-demand output, wrap-all behavior, entry prologues and facade placement, cross-chunk init definitions, and runtime-helper closure.
2. Every pre-existing explicit strict configuration in the scoped fixture suite has an on-demand counterpart. Output-sensitive cells snapshot both modes, while executed fixtures assert the same runtime behavior and tree-shaking result.
3. Directed fixtures cover retained barrel and namespace paths, emergent chunk cycles, CJS init exports, user/dynamic/emitted entry facades, legal TLA output, and external-module boundaries.
4. The external differential fuzzer compares normal source execution with generated wrap-all and on-demand output across ESM, CJS, mixed modules, minification, package side-effect metadata, namespace reads, and output formats.
5. Full Rust, Node, WASI, Vite, formatting, lint, and repository validation remain the merge gate.

The migration is complete only after `override_wrap_kind()`, `hoist_esm_wrapper`, and order-specific reads of interop wrapper fields are removed.
