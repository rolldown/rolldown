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
- Strict order analysis and entry-trigger facade creation require `strictExecutionOrder`. [Manual groups also use order wrappers to break cycles caused by awaited dynamic entry facades.](./implementation.md#pipeline)
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

A barrel whose retained statements are direct re-exports can have an interop ESM wrapper that
initializes all retained exports together, including bindings used only by a dynamic consumer. ESM-only consumer-local
routing bypasses that shared initializer only when the entire static dependency graph can
initialize independently of outside state. This proof is stronger than `moduleSideEffects: false`:
an initializer such as `export const snapshot = importedValue` has no write side effect, but
delaying it past a mutation changes its exported value. Calls annotated pure, getter reads and
coercions can observe the same timing difference.

The scanner proves a restricted set of local initializations: literal data, inert functions and
classes, and reads of already initialized local bindings. Function bodies and instance field
initializers are deferred; imported/global eager reads, calls, spreads, destructuring, computed
keys, class definition-time work and possible local TDZ reads reject the proof. A reverse
static-import worklist rejects every module depending on an unproven module, an external, a
synchronous cycle, TLA, a missing-export shim or an explicit effectful
module contract. Ordinary tree-shaking analysis remains separate: generated star-re-export glue
can mark a module effectful even when this source-level proof succeeds.

Eager consumers then initialize only the leaf bindings they use; a dynamic consumer initializes
its own leaves when loaded. The leaves retain their existing memoized module initializers, live
bindings and shared object identities. ESM-only route seeds require an interop `WrapKind::Esm`;
every ESM-only outer forwarder must satisfy the same complete dependency proof. CJS carrier
routes keep their separate record-based eligibility in the routing fixpoint. Direct `require()`
targets, inlined dynamic imports with code splitting disabled, and unproven graphs keep their
guarded module initialization.

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
inline mode as tree shaking, unless its declaring module or a dependency of that module has side
effects: tree shaking keeps that module, so the read still routes it. Module-global leaf or
namespace liveness is deliberately insufficient, because another importer can make the same
canonical symbol live without retaining it for this consumer.

The CJS-carrier optimization is restricted to modules whose retained body consists entirely of
direct re-exports. Tree shaking must be enabled, and the module must not be in a
synchronous `import`/`require` SCC, be the target of an opaque CommonJS `require()`, carry top-level await or a
TLA dependency, expose dynamic exports, or require a missing-export shim. CJS `export *` is also
excluded because its namespace is dynamic. These shapes keep the
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
its module-wide `load_dependencies` union is not traversed. The resolver selects leaves and pure
carriers for each incoming consumer. The unconditional waypoint walk retains recursively eager
carriers and inner routes without opaque namespace use. Symbol-derived dependencies and retained
namespace getters keep namespace owners reachable even when forwarding statements are removed.
Opaque namespace use can place leaf modules in a statically loaded shared chunk. A barrel that is
itself a user or dynamic entry exposes its complete namespace and uses the conservative full
traversal and an entry prologue containing every statically known target.

A wrapped mixed ESM module keeps its own guarded initializer for local code and mandatory dependencies.
Eligible direct re-export records route initialization from the binding consumer instead: importing
`LOCAL` from `export * from './values.js'; export const LOCAL = 'local'` calls only the mixed
module's initializer, while importing a forwarded binding also initializes its defining module.
The routing decision is per record, so evaluating the local body cannot discharge a later
consumer's demand for a forwarded binding. The existing module guards preserve one-time execution.
If on-demand wrapping leaves the mixed module unwrapped, a consumer that imports a forwarded
binding initializes that binding's defining module itself. A non-routed record whose definer is
order-wrapped has its only initialization site inside the mixed module's wrapper: an unwrapped
mixed module emits no `init_*` call for an excluded re-export, so a definer that no startup
consumer imports would evaluate when a lazy consumer first imports it, later than under wrap-all.
The on-demand plan therefore wraps every eager module whose wrapper would forward initialization
to a planned module (see [Emergent-cycle plan projection](#emergent-cycle-plan-projection-on-demand)).
`mixed_reexport_sensitive` and `mixed_reexport_grouped_definer` isolate such a definer with a
manual chunk group so placement cannot hide the hop.

A routed target must be side-effect-free and have no `ExecutionOrderSensitive` module in its static
or binding dependency closure, apart from the inert runtime helpers. Side-effect metadata alone is
insufficient: a pure initializer can snapshot an imported mutable value, and postponing that read changes the exported value. The
closure includes excluded forwarding modules and conservatively includes unused static records.
Interop wrappers, synchronous cycles, require targets, TLA, dynamic exports, missing-export shims,
and no-treeshake contracts retain their current initialization. Namespace-valued
re-exports and mixed modules with an opaque namespace consumer also retain the monolithic path;
namespace getters have their own static loading dependencies. A statically resolved namespace member
read can still select an ordinary forwarded binding.

Mixed modules keep their complete `load_dependencies` traversal. Their local bodies may depend on
imports even when forwarding work is consumer-local, so the empty-barrel shortcut does not apply.
Entry activation includes the retained forwarded targets followed by the mixed module's local
initializer, including activation at a collapsed dynamic-entry call site.

Monolithic initialization also participates in pre-chunk reachability. A binding flattened to its
defining module can leave an initializer dependency absent from `load_dependencies`; registering
that dependency after placement imports whichever lazy entry happens to host the target. The
pre-chunk pass uses Register's included-record gate and the final metadata pass's excluded-statement
collector to add the retained targets to the consuming entry's bits. Frozen re-export paths,
overlays, namespace demand, and transparent-wrapper ownership apply before placement as well as
during final emission. Cycle projection's broader record gate is unsuitable for these loading
edges: retaining an unused forwarding hop can load and execute a pure initializer too early in
on-demand mode. Link-owned liveness and interop decisions remain fixed.

The pass runs against the wrap-all probe in both modes. Under on-demand wrapping that plan is a
superset of the final one: a pure mixed module that ends up unwrapped still receives the bits of
every consumer that would call its wrapper under wrap-all, so two lazy entries that share only its
forwarded bindings load one common chunk that also carries its local body (`issues/11033`,
on-demand variant). Wrap-all placement is exact.

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
- **Included + retained excluded re-export forwarding** — a wrapped importer's included imports,
  retained excluded re-export hops, and excluded plain imports that declare bindings, via the shared
  `collect_wrapped_esm_init_targets_for_import_record`.
- **Non-included forwarder hops** — a wrapped importer's re-export of a _non-included_ forwarder,
  walking the forwarder's every static import (not just its resolved exports), the excluded-statement
  metadata routing.

Deliberately omitted, and why it is sound: interop wrapper edges (already in the baseline) and
nested/consumption-gated hops (owned by a wrapped ancestor, tree-shaken for an eager forwarder) are
skipped so the projection never over-wraps a tree-shaking-equivalent graph; entry-facade edges are
omitted because a facade holds zero modules, hence zero static indegree, and can never sit in a
static SCC (debug-asserted in the final link pass). The remaining over-approximation (it omits the
wrapping's own liveness suppression, and it keeps the records of a re-export-transparent wrapper,
which registration skips) only ever wraps more, which is always legal — wrap-all is the standing
proof.

The same fixpoint closes the plan over **eager forwarders of planned modules**. The final metadata
pass gives a wrapper the `init_*` calls of its tree-shaken re-export hops and retained re-export
paths; an eager module emits none of them, so an eager forwarder gives a planned definer no
initialization site at its own position (see
[Tree-shaking parity across strict modes](#tree-shaking-parity-across-strict-modes)). Each
eager-eligible module's would-be obligations are resolved once with that pass against a wrap-all
probe on the same chunk graph — the frozen re-export evidence assigns a hop to the first wrapped
barrel on its path, so only a probe that wraps the forwarder itself shows what its wrapper would
forward. Each round traverses these obligations in reverse from the planned modules, wrapping
every forwarder reached before rebuilding the plan. Forwarding chains therefore require no
per-hop chunk projection. Routed records contribute nothing (their definers are
time-independent by construction), and a transparent wrapper owns no calls, so a pure barrel is
never wrapped by this rule.

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

### Group chunk re-exports for merged emitted entries

An emitted `allow-extension` entry that merges into a group chunk can re-export a binding from another chunk. In this case, Rollup makes the group chunk import that binding and export it with the export name of the entry (`ensureReexportsAreAvailableForModule`). This approach caused these failures in a trial for [#11767](https://github.com/rolldown/rolldown/issues/11767):

- A CommonJS entry that forwards ESM exports (`module.exports = require('./dep.js')`) put an undeclared export into the group chunk. Thus Node could not load the group chunk.
- A re-exported `then` made the namespace of the group chunk a thenable (an object with a `then` method). Thus `import()` of a sibling entry returned the wrong value.
- The group chunk exported a binding of a wrapped ESM module, but the group chunk did not call the `init_*()` function of that module. Thus a program that imported only the group chunk got `undefined`.
- An ambiguous `export *` name gave the group chunk an extra import. The module-level import prediction (`predicted_static_import_targets`) did not include this import, so a debug assertion in `compute_cross_chunk_links` panicked.
- When one binding had two export names, the group chunk exported it with only one of the names.

Each failure needs its own guard. Also, the module-level import prediction must include the new imports. Thus the chunk optimizer keeps the facade of such an entry instead (see [implementation.md](./implementation.md#facade-elimination-optimize_facade_entry_chunks)). The cost is one more chunk for each such entry. A later optimization can export the aliases from the group chunk when the group chunk already imports their bindings. That optimization needs its own tests for names, initialization, cycles and a sibling `then` export.

A kept facade does not give the same value as the namespace object of the entry in the group chunk. `import()` of the entry returns that object when the facade is removed:

- In CJS output, a write to an export of the facade changes the export. A write to an export of the namespace object fails, as for a module namespace.
- The facade reads each CommonJS export one time, when it loads. If a getter throws, the export of the facade is `undefined`, as in Node. The namespace object reads the export only when code reads it. The two share one read (see **Shared reads** below).
- Another module can read the namespace object as a value (`import * as ns`). Then `import()` must return that same object.

Thus the facade stays for code outside the bundle, and each `import()` of the entry in the bundle reads the namespace object in the group chunk. `entry_module_to_entry_chunk` still names the facade, and a second map gives the target of `import()`. Then `import()` in the bundle gets the same value as when the facade is removed, and code outside the bundle gets the re-exports of the entry.

The redirect applies only when the cross-chunk check is the only reason for the facade. A facade that the name check keeps is still the target of `import()`. In strict execution order, an emitted entry in the order wrap plan keeps a facade in all layouts, so `import()` of such an entry also loads the facade.

**CommonJS properties.** A facade reads a CommonJS export when it loads. Then a getter can run one more time, or earlier than in Node. In a trial, a separate entry that forwarded such an export ran a CommonJS getter three times in ESM output, and its CJS output threw a `ReferenceError`. Node ran the getter one time. Thus a facade with a CommonJS property stays only if all reads of the module share one read, in the order of Node (see **Shared reads** below). If they cannot, the entry gets the result of the name check for all of its exports, as for an export of another kind. The same applies to an entry with an `export *` from an external module, because the check uses one flag (`has_dynamic_exports`) for both kinds of `export *`. A separate check for each kind is possible, but the issue does not need it.

**Early check.** The chunk optimizer decides the facades before the cross-chunk links exist. The shared-read pass needs these links, so it runs later, and it cannot change a facade decision. Thus an early check predicts the result of that pass for each CommonJS module (`commonjs_property_verdict`). It uses the chunks that the decisions of the optimizer give, and the facts of the module graph. A debug build asserts the prediction in the shared-read pass. The check rejects a module in these cases. Each case comes from a measured layout where a kept facade gave values that differ from Node:

- **Code reads the module outside a re-export.** The shared read reads all properties at its first call. A read in code reads the property again. In a measured layout, an entry reads `x` of a CommonJS module in its body, and a facade re-exports another property of the module. With the facade, the read in the body gave `4`. Without the facade, it gives `1`, as in Node. A static read of a member of a namespace import (`ns.x`) is also a read in code. With the facade, such a read gave `4`, and without the facade `3`. Node gives `1`.
- **Code can read a namespace object with a getter of the property.** A getter of the namespace object of a module that re-exports the property reads it at each access. In a measured layout, an entry passes a namespace import to a function that reads `x`. With the facade, the function read `4`, and without the facade `3`. Node gives `1`.
- **An `import()` gives such a namespace object.** The shared read covers the getters of the namespace object of an emitted entry whose facade the check keeps, and it covers the exports of an entry chunk. It does not cover the namespace object of another module that `import()` loads. In a measured layout, code reads `x` of the result of such an `import()`. With the facade, the read gave `4`. Without the facade, it gives `1`, as in Node.
- **A read has no path to the accessor.** The read then keeps its own read. When the shared-read pass decided alone, 28 results in the replay sets differed from Node where the bundle without the cross-chunk check gave the values of Node. All 28 were in layouts where the chunk of the CommonJS module is an entry chunk that `import()` loads, or in cycles of chunks.
- **A read uses the namespace of a barrel, or the order of the properties is unknown.** Then the pass cannot read all properties in one accessor in the order of Node (see **Shared reads** below).

The check makes one decision for each module, for all entries. It uses no facade decision of the optimizer. The reads of an emitted entry in a group count in both places where they can occur: in the group chunk, and in the facade. For an `import()` of such an entry, the check expects the facade only if each CommonJS property of the entry comes from the module that it checks (`kept_for_other_chunk`). Thus the check needs no fixpoint over the facade decisions. A module that the check rejects for one entry is rejected for all entries.

**A wrong prediction.** A release build does not check the prediction, and it does not correct a wrong prediction. If the prediction is wrong, the reads of the module stay separate, as in the first rejected alternative below. Then a getter can run two or more times, and an export can get another value than in Node. A wrong prediction that a test or a user finds is a bug in the early check, and the fix goes into the early check.

**Rejected alternative: keep each facade, and let the shared-read pass fall back to separate reads.** The pass knows the links, so its decision is exact. But a fallback runs a getter two or more times. In the replay sets, this gave the 28 results above that differ from Node. In 12 of them, an export has another value. In the other 16, only the number of getter calls differs. In the layout with a read in code, the read in the body gave `4` in 8 of 10 cases.

**Rejected alternative: reject a module when the output includes the namespace object of a module that re-exports its property.** This rule finds the namespace objects that code references. It does not find the namespace object that `import()` gives. In the layout with the `import()` above, the facade then stayed, and the read gave `4`.

**Rejected alternative: a second run of the facade decisions after a wrong prediction.** The shared-read pass can find a wrong prediction in a release build too. Then the bundle can run the chunk optimizer again from `optimize_facade_entry_chunks`, with the module rejected. But the passes between the facade decisions and the shared-read pass change shared state of the compiler. This state includes the symbols, the linking metadata, the used symbols and the chunk graph. A second run needs a copy of this state from before these passes. These types do not implement `Clone`, and the scope data of `oxc` has only a copy into another arena. A second run from an earlier point, for example `generate_chunks`, also calls the `test` and `name` functions of the `codeSplitting` groups again.

**Named imports from external modules in CJS output.** In CJS output, the linker makes such an import a property of the namespace of the external module. `namespace_alias_origin` finds the external module, and the check rejects the property. In a trial, an emitted entry re-exported `x` from an external module with a getter that throws, and it also exported `own = 17`. With `externalLiveBindings: false`, the group chunk loaded, and `own` was `17`. A forced facade wrote `exports.x = counter.x`, so its load threw. In ESM output, the facade imports the binding and does not read it when it loads.

**Rejected alternative: accept a property of an external module in CJS output.** Then the entry also keeps its facade in CJS output. But with `externalLiveBindings: false`, that facade can throw when it loads, as in the trial. A test with `node:path` does not show this risk, because `sep` has no getter.

**Rejected alternative: a facade with only the ESM exports of such an entry.** Code outside the bundle then gets an interface that matches neither the source nor the output of the name check.

**Rejected alternative: remove the facade of an `import()` target.** Then `import()` and a static namespace import get the same object without a redirect. But the entry loses its re-exports from other chunks for code outside the bundle, as in the issue.

**Load of a kept facade.** An ESM facade binds each export to a variable, so it reads each CommonJS export when it loads. When the optimizer removes the facade, the group chunk does not export such a property, and no file reads it when it loads. Code outside the bundle then imports the other exports of the entry from the group chunk, and a getter that throws does not stop that load. Thus a getter that throws must not stop the load of a facade that the cross-chunk check keeps. Node reads each export of a CommonJS module one time when it imports the module, and it ignores an error from a getter. Thus such a facade reads each CommonJS export one time, in a `try` block, and all export names of one CommonJS export get the value of that read (`export { x as a, x as b }`). The CJS facade does the same, so both output formats give the same values. Module initialization stays outside the `try` block.

The early check accepts only a property of one CommonJS module (`namespace_alias_origin`). It rejects a property of an external module and a property of an unknown origin. Thus a marked facade saves only the values of CommonJS properties. A debug assertion checks this when the facade renders (`is_marked_facade_property`).

Another chunk can export the same CommonJS property, for example an ordinary entry that also re-exports it. Node reads the property one time for both entries, so the chunks share one read (see **Shared reads** below). Other entry chunks that do not share a read do not change. In ESM output, they read without a `try` block, so a getter that throws makes their load fail. CJS output keeps their getters. This design does not change these separate gaps. In strict execution order, the bundle also restores the facade of an emitted entry in the order wrap plan when the cross-chunk check does not keep it. Thus a kept facade of such an entry renders as a restored facade, and it does not share a read. In a measured set, such a facade and another entry then ran a getter two times, and Node ran it one time.

**Rejected alternative: a `try` block in each CJS getter.** Then the spread of the facade (`{ ...facade }`) does not throw. But the getter reads the export at each access, so two reads can give different values. Node reads the export one time.

**Rejected alternative: no CommonJS exports in such a facade.** The facade then reads nothing when it loads, as the group chunk. But code outside the bundle then loses an export that the source has.

**Rejected alternative: no facade for each entry with a CommonJS property.** The group chunk then loads as it does without the cross-chunk check. But each such entry loses its re-exports from other chunks again ([#11767](https://github.com/rolldown/rolldown/issues/11767)). The bundle gives this result only to an entry that the early check rejects.

**Shared reads.** When a facade and another chunk export the same CommonJS property, they call one accessor. The chunk of the CommonJS module or of the namespace declares it (see [implementation.md](./implementation.md#shared-commonjs-property-reads)). These rules decide the design:

- No runtime change. The accessor is code of the chunk, not a runtime helper.
- No new chunk dependency. A chunk that does not import the owner of the accessor calls it through a forward in a chunk between them. The pass adds no import and no namespace extraction to get a path. It also removes no import.
- No new visible key. A chunk gets the accessor or a forward only if the new export changes neither a fixed entry signature nor the namespace that `import()` gives for the chunk. This applies to a pure dynamic entry, and also to a user-defined or emitted entry that code in the bundle loads with `import()`. `allow-extension` permits new exports of an entry file, but `Object.keys(await import('./app.js'))` can see a new key. A forward can stay when the importers read an extracted namespace that hides it. Runtime placement can add a key to such a namespace when its importers cannot read an extracted namespace (`dynamic_entry_extraction_skip_reason`). A shared read cannot. Such a namespace can already have extra keys, but that is not a reason to add one more key.
- A reader that has no path through such chunks reads a public export of the entry chunk that declares the namespace of the read, if that entry chunk can call the accessor. The reader imports the namespace from that chunk, so the edges stay the same. The export has a name of the entry, so the namespace gets no new key.
- The binding must have its value when the reader reads it. Thus the entry module must run before the exports of its chunk: the module is not wrapped, or the chunk calls its ES module wrapper before the exports. Also, the entry chunk must not import the chunk of the reader. In a cycle of chunks, the reader can read the binding before the exports of the entry chunk run. An import of the entry chunk alone does not prove the order.
- If a reader cannot read such an export, the early check rejects the module, and the entry gets no facade. Otherwise the reader keeps its own read, and the getter runs two times. Then the load order decides which reader gets the second value.
- The facade and the namespace object of its entry are two ways to load the entry: code outside the bundle loads the facade, and `import()` in the bundle gives the namespace object. Thus a getter of that namespace object is a reader too. It calls the accessor when code reads the export, not when the namespace object is created. In each load order, the facade and each read of the namespace object give the value of one read, as in Node.
- The read runs at the first use. An import of the owner proves that the owner loads. It does not prove that the getter must run at that time. Thus the accessor reads the property at its first call, not when the owner loads.
- A getter that throws gives `undefined`, and the accessor does not read again, as in Node. A namespace that is `null` or `undefined` is not a read of the property, so the accessor throws, as a plain property read does.
- The value is saved. In CJS output, the getter of an ordinary entry that shares a read returns the saved value after the first read. This also applies when only that entry loads, so two reads of its export give the same value, as in Node. Before, the getter read the property at each access, so it saw each later write. If the getter read the property again, the CommonJS getter could run a second time, and the ordinary entry could give another value than the facade. The saved value matches Node after the snapshot, but the read can run later than in Node. Node copies the named exports of a CommonJS module when it imports the module ([CommonJS namespaces](https://nodejs.org/api/esm.html#commonjs-namespaces)). The output reads the property at the first call of the accessor, and an ordinary CJS entry and a namespace object call it at the first access of a getter. Thus a write before the first read can be visible. A write after the first read does not change the saved value.
- Only a CommonJS named re-export that a marked facade shares gets a saved value. Direct `require()` property reads, the bindings of an external ES module, the getters of other namespace objects, and ordinary entries without such a facade do not change.
- The order of the reads of one module is the order of Node. When an ES module imports a CommonJS module, Node reads all export names of the module one time, in source order. A getter can change the value of another property. If each key has its own accessor, the chunk that loads first reads its properties first, so a facade that loads first can change an existing export of an ordinary entry. Thus one accessor reads all properties of a module that the chunks read, if a facade reads one of them. An entry that reads one property also runs the other getters, as in Node. Its values follow the source order of the module, not the order of the loads. For example, if the getters of `x` and `y` share a counter and the source assigns `exports.y` first, Node gives `x = 2`, and so does the output.
- The order must be known. The bundler uses the rules of the static analysis of Node to find the first place of each name. Node finds a name in a call of `Object.defineProperty` only for some forms of the getter. The bundler accepts only the forms that the results of Node show, in a plain source text. A place of another form, or a place with parentheses, escapes, comments or characters that are not ASCII, is unsure. If the first place of a name is unsure, the order is unknown. Then the early check rejects the module, and the entry gets no facade.
- One accessor reads all keys of the module or no key. A read that cannot call the accessor reads a public export of an entry chunk that calls it, with the rules above for a reader that has no path. If a read can do neither, the early check rejects the module. An accessor for only some keys changes when the other getters run, also when no facade loads.
- If the chunk of the module cannot export the accessor, only the reads in that chunk call it. Each other read must read a public export of that chunk. The accessor then stays private, so the namespace of the entry gets no new key.

**Rejected alternative: one accessor for each key, in the order of the loads.** Then a facade that loads first can change an existing export of an ordinary entry, for example from `1` to `2`.

**Rejected alternative: one accessor for the keys that can call it.** A key that keeps its own read then runs its getter outside the order of the accessor. In a measured layout, the getters of `x`, `y` and `z` share a counter, and the source assigns `x`, `z` and `y` in that order. An ordinary entry exports `x` and `z`, and it loads alone. The accessor reads `x` and `y`, and then the entry reads `z`, so the entry gets `z = 3`. Node and the reads of each key give `z = 2`.

**Rejected alternative: no module accessor when the chunk of the module cannot export it.** The other reads can still read public exports of that chunk. Without the module accessor, each property keeps its own read, so the getters run in the order of the reads. For example, Node reads `z`, then `x`, and the getters share a counter. An entry that loads alone and reads `x` first then gets `x = 1`. Node gives `x = 2`.

**Rejected alternative: no public export of a wrapped entry.** A `require()` of the entry, or of a module that imports the entry, wraps the entry module. Then a facade that reads a property through that entry has no route, and all keys of the module keep their own reads. In a measured set of 84 cases with two groups (source orders, load orders and output formats), this rule gave values that differ from Node in 8 cases. The chunk calls the ES module wrapper of the entry before its exports, so the binding has its value when the chunk has run. With the binding, all 84 cases give the values of Node.

**Rejected alternative: a cycle check only for a wrapped entry.** In a cycle of chunks, a reader can read a binding of an entry that is not wrapped before the exports of the entry chunk run. In a measured layout, the entry module reads `z` of the namespace object of an emitted entry in its body. The chunk of that namespace object imports the entry chunk. With a public export, the getter gave `undefined`, and Node gives `1`. Thus the check applies to each entry chunk. The check is conservative: it also refuses the binding in the load orders where the entry chunk runs before the reader. In a measured layout, a module in the chunk of the namespace object reads `z` of that object at its top level. With the check only for a wrapped entry, 54 of 72 cases gave the values of Node, and the other 18 cases read `undefined` in the cycle. With the check for each entry chunk, the early check rejects the module, and the entry gets no facade. Then the 18 cases throw a `TypeError` in the cycle, also without the cross-chunk check.

**Rejected alternative: an unsure place is not a place.** Then a later place gives the position of the name. But Node can find the name at the unsure place. Then the position is too late, and the accessor reads the names in an order that is not the order of Node. With an unknown order, the early check rejects the module.

**Rejected alternative: positions from the scan stage.** The scanner could record the positions of the export names of each CommonJS module. But only a module that the chunks read in two or more properties, with a facade, needs them. Thus the early check and the generate stage walk the AST of a module only after the other conditions select it.

**Rejected alternative: a runtime helper that saves the value.** It changes the runtime for one case. A function of the chunk does the same work.

**Rejected alternative: a saved value at the top level of the owner.** The owner can load for a file that reads no property, for example for an entry that imports another export of the group chunk. Then the getter runs at that load.

**Rejected alternative: share only when each exporter imports the owner.** An exporter can get the namespace from another chunk, for example from the group chunk or from an entry chunk. Then each chunk reads the property. In CJS output, the value of the ordinary entry then changes when the facade loads first.

**Rejected alternative: a forward in an entry chunk whose namespace `import()` gives.** Runtime placement allows a new export in a pure dynamic entry whose importers cannot read an extracted namespace, and `allow-extension` allows a new export in a user-defined entry. But the forward is then a new key of `Object.keys(await import('./app.js'))`.

**Rejected alternative: a new import or a new extraction for a reader that has no path.** The reader then reaches the accessor, and the getter runs one time. But the change adds a chunk dependency, or it changes the value that `import()` gives, for one shared read.

**Rejected alternative: a public export of any entry chunk that the reader imports.** A reader can import two entry chunks that export the property. If the reader imports the export of the chunk that does not declare its namespace, the reader can lose its import of the chunk of the namespace. That removes a chunk edge.

**Rejected alternative: a note about the namespace getter.** When the facade loads first, a getter that reads the property again makes `ns.first` give another value than the facade. Node gives the same value, and so does the output when the optimizer removes the facade. A note does not remove this regression.

**Rejected alternative: a call of the accessor when the namespace object is created.** Then an `import()` that reads no export runs the getter.

**Unresolved: entries that the early check rejects.** Such an entry gets no facade, and code outside the bundle does not get its re-exports from other chunks, as without the cross-chunk check. The measured cases are:

- Code reads the module outside a re-export, also when the getters have no side effects. In a measured layout, an entry reads `keep` of a CommonJS module in its own code and re-exports `x` of the module. The entry loses `x`. A facade without the rule gives the value of Node, because the properties are plain values.
- Code can read the namespace object of a module that re-exports the property, or an `import()` gives it.
- A read has no path: the chunk of the module is an entry chunk that `import()` loads, a cycle of chunks, or a group chunk that holds an entry with dynamic importers.
- A read uses the namespace of a barrel, for example an entry with `export * from './lib.cjs'` that another entry re-exports.
- The order of the module is unknown.
- A barrel gets the property from two or more CommonJS modules.
- In CJS output, the property comes from an external module.

Compared with the first alternative above (keep each facade), the early check gives up 89 results with the values of Node in the replay sets. It also gives up 18 results in a second set of layouts, all because of the barrel rules. It removes the 28 results that differ from Node. In the fixtures, 16 of its rejections, counted for each build, are modules whose reads the shared-read pass can share.

**Unresolved: options for more facades.** Each option needs new measurements:

- A fixpoint over the facade decisions. The check treats each chunk that holds an entry with dynamic importers as a chunk that cannot get a new export. But an `import()` of an emitted entry whose facade the check keeps loads the group chunk, and then the shared-read pass does not count that chunk as a namespace that `import()` gives (`allow_extension_cross_chunk_reexport_shared_read_namespace`). A fixpoint can accept such modules, but the decision of one entry then depends on the decisions of the other entries.
- One property with one reader needs no accessor, so it needs no path. In the fixtures, two rejections of the early check, counted for each build, are candidates for this rule: `lib.cjs` in `allow_extension_cross_chunk_reexport_cjs_namespace`, and `counter.cjs` in `allow_extension_cross_chunk_reexport_shared_read_inline_const`. In each of them, the check finds no path for one property. If the facade stays, the shared-read pass counts one read of the property. But the early check does not know all readers. An `import()` of the entry can give the namespace object of the entry, and a getter of that object is a second reader. The check must count these getters before it can use this rule. Each of the two cases also needs a test of its values and of its getter count at run time.

**Unresolved: the prediction.** The early check does not know the inline common chunk records, because the selection of the records runs later. No fixture checks a marked facade in a build with records. Two conditions of the early check have no fixture: a read in code of a property that a barrel gets from two or more CommonJS modules, and a module or a namespace that is in no chunk. The edges of the early check do not include an import of a common chunk that gets the runtime after the check (`try_merge_runtime_chunk`). No test checks whether a path through such an import changes the result of the check. For one property, the debug assertion counts the accessor as one read, and each read that does not call the accessor as one more read. No input in the tests and in the replay sets has both kinds of read, so no test checks that part of the count.

**Unresolved: shared reads.**

- Two ordinary entries without such a facade still read a property one time each.
- In strict execution order, a restored facade reads the property in its own code, and the other entry reads it again. This also applies to a kept facade in the order wrap plan.
- In ESM output without an accessor, an entry that exports the property under two names reads it one time for each name. A reader of one of these exports gets the value of one of these reads.
- In CJS output, a chunk fills its exports object at its end. In a cycle of chunks, a read through the exports object of a chunk that has not reached its end fails, also without a shared read. Thus the test of the cycle uses only ESM output.
- The bundler identifies only some of the forms that Node finds. A form that Node finds, and that the bundler does not identify, gives an unknown order, and the early check rejects the module. Examples are a getter with a comment, and an escape in a name, as in `exports['y'] = 0`. A full model of the CommonJS lexer of Node can give a known order to more modules.

**Unresolved.** The namespace object of the group chunk differs from Node in two cases, with or without the facade:

- In ESM output, the namespace object does not get the names of an external `export *`.
- The namespace object reads a CommonJS export at each access, unless its getter shares the read of a facade. Node reads the export one time, when it loads the CommonJS module. Thus the namespace object sees a later write.

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

Generate-stage finalization stores order wrappers from strict execution-order planning and eligible manual-group imports in `OrderWrapState`.

```rust
pub struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, OrderWrappedModule>,
  reexport_init_transparent: FxHashSet<ModuleIdx>,
  consumer_local_reexport_routes: FxHashSet<ModuleIdx>,
  consumer_local_reexport_records: FxHashMap<ModuleIdx, FxHashSet<ImportRecordIdx>>,
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
- consumer-local re-export records can remain in the table even when no wrappers or import overlays are needed.

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
  pub member_read_star_reexport_paths: &'a [MemberReadStarReexportPath],
  pub kept_inlined_constant_reads: &'a FxHashSet<SymbolRef>,
  pub used_symbol_refs_builder: &'a UsedSymbolRefsBuilder,
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

After wrapper selection, `compute_wrapped_esm_init_metadata` derives the two facts that depend on both link and order state: whether an `init_*()` call is a no-op, and which wrapped modules must be initialized at each excluded statement. Interop and execution-order wrappers share one sealed result for a chunk layout. `finalize_chunk_plan` sweeps unused runtime helpers before deriving this result and replaces it if signature-facade creation changes the layout:

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

Lowering precomputes the complete, source-ordered namespace target list for every consumer-local route. Included import/re-export records and namespace-retained excluded `export *` records use the same importer-local resolver, including routes with a link-stage interop wrapper. Each star record selects only names it contributes to the importer's non-ambiguous export surface; `default`, shadowed names, and conflicting names create no forwarded initialization demand. A mixed module's own initializer covers its local body and mandatory dependencies, so an opaque outer namespace consumer must also initialize the forwarded bindings it exposes. Calls stay inside the consuming wrapper's evaluating guard and at the re-export record's source position. Entry rendering and collapsed dynamic-entry call-site activation consume the cached complete list: a consumer-local barrel entry's prologue or rewritten `import()` calls every namespace target. For a cross-chunk rewrite, the host chunk imports and re-exports every target wrapper/carrier beside the simulated namespace; the callback invokes them in list order before returning that namespace. Other order-wrapped entries emit an explicit init call, and interop entries used internally keep an inert implementation chunk behind their public facade.

### Topology

`OrderWrapState` drives module and runtime placement. Strict entry facades can also change topology without an order wrapper. `finalize_chunk_plan()` remains the boundary after which topology-derived metadata is final.

## Data Flow

```text
link + tree shaking
  -> immutable LinkingMetadata and execution dependencies
  -> pre-chunk order probe + retained init edges + importer-local entry-bit propagation
  -> provisional ChunkGraph
  -> OrderAnalysis / OrderWrapPlan
  -> lower plan into OrderWrapState
  -> sweep unused runtime
  -> derive sealed init metadata and cross-chunk links
  -> preserve strict entry signatures with public facades
  -> rederive init metadata and links if facades changed topology
  -> seal liveness and commit cross-chunk links
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
- Every ordinary-import init obligation corresponds to a link-stage execution dependency or to importer-local binding demand (a used binding, a statically folded member read, or a re-exported facade a downstream consumer reaches).
- Every excluded-statement init obligation is a retained re-export obligation, a synthetic obligation backed by an execution dependency, or a binding-demand obligation of an excluded plain import; a binding-less excluded plain import routes nothing.
- Final cross-chunk registration and finalizer emission require the same `Sealed<FinalEsmInitMetadata>` type; pre-final projection cannot supply one and does not consume final metadata.
- Wrap-all and on-demand preserve the same link-stage statement and binding liveness; only their wrapper plans may differ.
- Emit, Register, Project, and pre-chunk placement resolve consumer-local records through the same target model.
- Every order-wrapped entry has an explicit entry trigger.
- Strict ordering can create entry-trigger facades. The grouped dynamic import path removes eligible empty facades. Signature facades obey `preserveEntrySignatures` independently.

## Verification

Verification stays at observable compiler boundaries rather than reaching into private pass state:

1. Real-`Bundler` integration invariants cover flag-off legacy output, byte-identical hazard-free on-demand output, wrap-all behavior, entry prologues and facade placement, cross-chunk init definitions, and runtime-helper closure.
2. Every pre-existing explicit strict configuration in the scoped fixture suite has an on-demand counterpart. Output-sensitive cells snapshot both modes, while executed fixtures assert the same runtime behavior and tree-shaking result.
3. Directed fixtures cover retained barrel and namespace paths, emergent chunk cycles, CJS init exports, user/dynamic/emitted entry facades, legal TLA output, and external-module boundaries.
4. The external differential fuzzer compares normal source execution with generated wrap-all and on-demand output across ESM, CJS, mixed modules, minification, package side-effect metadata, namespace reads, and output formats.
5. Full Rust, Node, WASI, Vite, formatting, lint, and repository validation remain the merge gate.

The migration is complete only after `override_wrap_kind()`, `hoist_esm_wrapper`, and order-specific reads of interop wrapper fields are removed.
