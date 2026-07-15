# Module side effects

## Summary

`DetermineModuleSideEffectsPass` computes the final `DeterminedSideEffects` value for every module without reading or mutating `LinkStage` or linking metadata. It runs immediately after lazy-export normalization, when dynamic-export and wrapper facts are final, and returns a sealed dense `ModuleSideEffects` artifact.

The extraction deliberately preserves the existing depth-first algorithm. This pass does not fix cycle behavior, introduce an SCC analysis, compute a fixed point, or parallelize the graph walk.

Source: `crates/rolldown/src/stages/link_stage/passes/determine_module_side_effects.rs`.

## Pipeline placement

```text
ComputeDynamicExportsPass ───────────────────────────────────────┐
PlanModuleWrappingPass → CreateWrapperDeclarationsPass           │
  → NormalizeLazyExportsPass → final ModuleWrappers ─────────────┤
                                                                 ▼
                                            DetermineModuleSideEffectsPass
                                                                 │
                                                    sealed ModuleSideEffects
                                                                 │
                                                                 ▼
                                            compatibility projection of normal slots
```

`NormalizeLazyExportsPass` is the last operation that can invalidate a wrapper declaration, so side-effect analysis must read `ModuleWrappers`, not the earlier wrapper seed, plan, or declaration draft. `DynamicExports` remains sealed from its producer through this final typed consumer. After side-effect projection, the driver consumes formats and wrappers into the existing module and metadata fields, projects dynamic-export bits, and releases all four compact representation facts.

## Pass contract

| Slot            | Type                                  | Purpose                                                                          |
| --------------- | ------------------------------------- | -------------------------------------------------------------------------------- |
| `InputRead<'a>` | `DetermineModuleSideEffectsInput<'a>` | Borrows only `ModuleTable`, sealed `DynamicExports`, and final `ModuleWrappers`. |
| `InputOwned`    | `()`                                  | The pass mutates no entity table.                                                |
| `OutputRead`    | `ModuleSideEffects`                   | A dense `IndexVec<ModuleIdx, DeterminedSideEffects>` sealed by the harness.      |
| `OutputOwned`   | `()`                                  | No mutable domain continues from the pass.                                       |
| `Error`         | `Infallible`                          | The external link path remains infallible.                                       |

`ModuleSideEffects` exposes only its module count and a read-only `get(ModuleIdx)` operation that copies the small enum value. It has no constructor, iteration-order override, mutable access, clone, or consuming unwrap. The driver walks raw module order, copies only normal-module slots into the unchanged legacy field, and then explicitly drops the sealed artifact. External-module fields are not rewritten, matching the previous method.

## Exact algorithm

The pass first copies every normal and external module's initial `DeterminedSideEffects` enum into the dense output. It allocates a private dense cache with three states:

- `None`: this module has not been visited.
- `Visited`: this module is on a current recursion path, or returned through a branch that the legacy implementation intentionally does not memoize.
- `Cache(value)`: a normal module that started as `Analyzed(false)` has completed analysis.

The outer loop walks raw physical `ModuleIdx` order. For each slot, the recursive helper behaves as follows:

1. `Cache(value)` returns the memoized enum.
2. `Visited` returns the current dense output slot. This preserves the previous cycle back-edge behavior rather than solving the cycle.
3. `None` changes to `Visited` before examining the module.
4. `Analyzed(true)`, every `UserDefined` value, and `NoTreeshake` return unchanged and leave the cache at `Visited`.
5. External `Analyzed(false)` returns unchanged and also leaves the cache at `Visited`.
6. A normal `Analyzed(false)` module walks its physical `import_records` order, skips unresolved records, and recursively visits every resolved kind, including dynamic import, require, CSS-shaped kinds, `new URL`, and `HotAccept`.
7. Each record first recurses into its importee. A side-effectful dependency short-circuits the remaining records immediately.
8. Only after a side-effect-free recursive result does the export-star special case run. It applies only to `ImportKind::Import` records marked `IsExportStar` whose importee is normal. A final CJS or ESM wrapper returns `true`; an unwrapped importee returns its `DynamicExports` bit.
9. Only a completed normal `Analyzed(false)` computation writes `Cache(Analyzed(result))`.
10. The outer loop writes the returned value into the dense output only for a normal module.

Module order, record order, recursive short-circuiting, and the asymmetric cache writes are observable in cyclic graphs. They are part of the compatibility contract for this extraction.

## Why this pass is serial

The current algorithm shares one evolving cache and observes earlier outer-loop results on cycle back-edges. Independent per-module work would not preserve that state, and a parallel SCC or fixed-point implementation would intentionally compute different answers for some cycles. The pass therefore remains serial. Any future algorithm change requires its own compatibility decision, fixtures, and performance evidence; it is not a mechanical consequence of adopting the pass harness.

## Coverage

Focused tests pin:

- both record orders and both physical module orders for a cycle plus a side-effectful sibling, directly pinning the order-dependent `[true, false, true]` and `[true, true, true]` results;
- transitive wrapped export stars and final CJS, ESM, and cleared `None` wrapper states;
- unwrapped dynamic-export stars;
- external modules and the normal-importee guard;
- every resolved `ImportKind`, including `HotAccept`;
- unresolved-record skipping;
- the exact export-star predicate; and
- preservation of `UserDefined`, `Analyzed(true)`, and `NoTreeshake` enum variants.

The production trace test pins the pass after `NormalizeLazyExportsPass`. Broader link, Node, Rollup compatibility, deterministic-output, WASM, timing, and memory gates remain part of the pass-pipeline validation rather than being duplicated here.

## Related

- [Pass-based pipeline implementation](../../pass-based-pipeline/implementation.md)
- [Determine module formats](../determine-module-exports-kind/implementation.md)
- [Reference needed symbols](../reference-needed-symbols/implementation.md)
