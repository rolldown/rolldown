# Cross-Module Optimization

## Summary

`CrossModuleOptimizationPass` is pass 22 of the twenty-four-pass Link pipeline. It runs immediately after `ReferenceNeededSymbolsPass`, owns the statement table and global-constant draft, analyzes normal modules in parallel, commits mutations deterministically in `SortedModules` order, seals the short-lived unreachable-dynamic-import fact, and finalizes the global constants for `TreeShakePass` and the final compatibility adapter.

The extraction preserves the existing optimization algorithm and its compatibility issues. It changes the representation and lifecycle boundary, not the user-visible optimization policy.

Sources:

- `crates/rolldown/src/stages/link_stage/passes/cross_module_optimization.rs` — pass contract, validation, bounded round scheduling, deterministic commit, and focused tests.
- `crates/rolldown/src/stages/link_stage/cross_module_optimization_runner.rs` — the per-module AST visitor and its narrow analysis result.

## Pipeline Placement

The typed tail is:

```text
ResolveMemberExpressionsPass
  → CollectEntryExportRootsPass
  → CreateSyntheticExportStatementsPass
  → ReferenceNeededSymbolsPass
  → CrossModuleOptimizationPass
  → TreeShakePass
  → FinalizeModuleDependenciesPass
  → LegacyOutputAdapter
```

N is the final typed reader of `DynamicExports` and `CjsNamespaceMerges`, but it is not the final reader of formats, wrappers, or side effects. P keeps `EntryPlanDraft`, `SortedModules`, and `MemberExprResolutions` typed past N, borrows the final `SymbolRefDb`, `IndexEcmaAst`, and `ModuleTable`, and takes ownership only of the two domains it can change: `IndexStmtInfos` and `GlobalConstantsDraft`. H later reads final formats, wrappers, exports, routing, constants, and export chains; G is the final reader of side effects, member resolutions, and entry roots.

After P returns, H consumes `EntryPlanDraft` and borrows `GlobalConstants`, sealed `UnreachableDynamicImports`, and the remaining typed inclusion facts. G then consumes the dependency and runtime-helper drafts. Only after G does the driver drain `PassPipelineCtx` and invoke `LegacyOutputAdapter`; the adapter moves `SortedModules`, converts final constants, projects member resolutions and resolved exports, and constructs the legacy output once.

## Pass Contract

| Slot            | Type                               | Contents and lifecycle                                                                                                                                                                |
| --------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `InputRead<'a>` | `CrossModuleOptimizationInput<'a>` | Borrows `ModuleTable`, `IndexEcmaAst`, `SymbolRefDb`, `SortedModules`, `EntryPlanDraft`, and `MemberExprResolutions`; copies `FlatOptions`; borrows `SharedNormalizedBundlerOptions`. |
| `InputOwned`    | `CrossModuleOptimizationOwned`     | Moves the unique `IndexStmtInfos` and `GlobalConstantsDraft` into P.                                                                                                                  |
| `OutputRead`    | `UnreachableDynamicImports`        | Sealed by the harness. It exposes only `contains(ModuleIdx, NodeId)` for inclusion.                                                                                                   |
| `OutputOwned`   | `CrossModuleOptimizationOutput`    | One-call envelope containing the updated statement table and final `GlobalConstants`; the driver destructures it immediately.                                                         |
| `Error`         | `Infallible`                       | P emits no diagnostics and preserves Link's infallible boundary.                                                                                                                      |

`GlobalConstantsDraft` has the read and extension operations required during iterative discovery. P consumes it into `GlobalConstants`, which exposes only the narrow lookup H needs and the final consuming compatibility conversion used by the adapter.

## Round Schedule

Configuration preserves the legacy arithmetic exactly:

```text
inline rounds = options.optimization.inline_const_pass() - 1
forced rounds = 1 when any normal module has TopExportedSideEffectsFreeFunction, otherwise 0
round limit = max(inline rounds, forced rounds)
inline extraction enabled = inline rounds >= 1
```

The zero-round path returns before dense-layout validation, related-import map construction, or any module-table traversal beyond configuration. This preserves the disabled common path and its existing tolerance of otherwise unused malformed fixtures.

For an active invocation, P first validates the complete layout and builds the related dynamic-import map. The first round analyzes every normal module in `SortedModules`. A later round analyzes only modules whose named imports canonicalize to one of the constants newly discovered in the previous round. New constant refs themselves are not canonicalized before this filter, matching the previous implementation.

Every round borrows one immutable snapshot of `GlobalConstantsDraft` for all parallel module analyses. A constant found by one module is therefore invisible to every other module in the same round. The parallel result retains one optional slot per sorted module; the serial commit zips those slots with `SortedModules`, replaces statement evaluation flags, extends constants, and unions unreachable NodeIds in deterministic module order.

Only a nonempty local constant batch keeps the loop running. Evaluation-flag changes and new unreachable nodes do not request another round, and the configured bound may stop before a semantic fixpoint. This preserves the legacy bounded-round limit and local-constant early exit.

## Per-Module Analysis

The sibling runner receives only one normal module, its AST, shared module and symbol facts, the current constant snapshot, copied options, that module's related dynamic-import identities, and its member-expression resolutions. Keeping the AST visitor outside the pass subtree lets the subtree inventory continue to require exactly one direct `Pass` implementation per pass declaration without admitting an unrelated `Visit` implementation.

The visitor preserves these behaviors:

- Bare identifier calls and fully resolved member calls can use `SideEffectsFreeFunction` only when the canonical binding is not reassigned. A member resolution with trailing properties is not treated as the called export.
- A statement containing a newly recognized side-effect-free call is reanalyzed, and its tree-shaking flags replace the previous flags rather than being ORed into them. Execution-order sensitivity remains the scan-stage fact.
- `PureAnnotationOnly` calls affect statement flags but do not establish the lazy callback path used to mark related dynamic imports unreachable. Empty side-effect-free functions do establish that path.
- The previous call-tracker behavior remains: when a side-effect-free call had no earlier tracked parent, the latest node remains installed after the visit.
- Named exported constants are extracted only when inline-constant optimization is active. Default-export literal extraction still runs with inline extraction disabled when the module has tracked related dynamic imports.
- Namespace binding symbol IDs are reconstructed from `NamedImport { imported: Specifier::Star, .. }` so statement reanalysis preserves namespace-read semantics.

## Fail-Closed Active Boundary

The active path validates before mutation:

- AST, statement, symbol, and member-resolution domains have exactly the module-table length.
- Every normal slot has matching embedded identity, AST, symbol database, member-resolution map, and at least `body.len() + 1` statements; every external slot has matching identity, no AST or member resolutions, a symbol database, and only the namespace statement.
- Every `SortedModules` value is an in-range normal module with an AST and member-resolution slot, and no value repeats. Reachability coverage is intentionally not re-derived here.
- Every existing global constant is owned by an in-range normal module with a symbol slot.
- Every related dynamic import is grouped under its exact in-range normal entry and has a normal importer. The module loader excludes external dynamic targets when it creates dynamic entries, while user-defined and emitted entries go through `load_entry_module()`, which rejects external modules; P fails closed if that producer invariant is violated.
- The related statement must contain the related record. The related record and the record selected by the import expression's NodeId map must both exist, be listed on that statement, be dynamic imports, resolve to the grouped target, and carry the exact statement and NodeId identity. Their physical record indices may differ because the scanner can retain equivalent duplicate records for one import expression.

Physical record-index equality is deliberately not an invariant. The `inline_empty_function_call/basic` production fixture contains two equivalent dynamic-import records for one NodeId: `EntryPoint::related_stmt_infos` retains the first while `NormalModule::imports` points at the later record. P validates both semantic identities independently and preserves the related NodeId map used by the legacy algorithm.

## Preserved Compatibility Issues

This representation-only extraction intentionally leaves I-072 unchanged:

- `inline_const.pass = 0` underflows at `inline_const_pass() - 1` in debug builds and wraps in release builds.
- Default-export literal extraction can still occur while inline extraction is disabled when related dynamic imports are tracked.

These behaviors need separate semantic fixes and fixtures; changing them here would make it impossible to distinguish a pipeline migration from an optimization-policy change.

The development partial-rebuild path can also retain stale `related_stmt_infos` from scan-cache reuse. P's active validation may expose that pre-existing identity mismatch earlier than the old code. Fixing the scan-cache lifecycle is outside this Link-only extraction.

## Coverage

Focused unit tests pin the pass-count rules, zero-round boundary, immutable same-round constants, later-round named-import filtering, existing CommonJS constants, identifier and namespace-member calls, evaluation-flag replacement, empty versus annotation-only callbacks, unreachable dynamic imports, equivalent duplicate records, rejection of external entry roots, default literal behavior, sealed output, and every active dense-slot and related-import identity check. The production trace pins P immediately after N, and the pass-subtree inventory pins its narrow source shape and forbidden-carrier boundary.

Broader pass, fixture, Clippy, and target checks remain part of the pass-pipeline validation matrix. Timing and RSS are measured only on the complete final Link structure; intermediate structural trees are not performance acceptance candidates.

## Related

- [Pass-based pipeline implementation](../../pass-based-pipeline/implementation.md)
- [Reference-needed symbols](../reference-needed-symbols/implementation.md)
- [Module execution order](../module-execution-order/implementation.md)
