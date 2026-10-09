# App Scenario

## What is App Scenario

`scenario: 'app'` (proposed — not yet implemented) tells rolldown the output is an application consumed by browsers. This switches rolldown from its general-purpose defaults to a mode optimized for loading performance.

```js
export default defineConfig({
  scenario: 'app', // default: 'general'
});
```

> **Scope:** `scenario` is a top-level (build/input) option, not an output option. This is because it forces settings like `preserveEntrySignatures` which are build-level — they affect module linking, not just output rendering. In a multi-output config, `preserveEntrySignatures` is shared across all outputs, so scoping `scenario` under `output` would cause one output's setting to silently alter siblings. Keeping it at the top level makes the blast radius explicit.

## Why a dedicated scenario

General-purpose bundler optimizations don't scale. Each optimization is isolated — you can only do A if B is enabled, and each combination has its own edge cases, correctness constraints, and implementation cost. The result: enormous engineering effort for incremental improvements that don't meaningfully benefit the primary use case (bundling web apps).

A dedicated scenario also reduces maintenance overhead. Instead of supporting every combination of options and spending effort on niche/less common use cases, we focus on the config set that matters most. Fewer code paths to maintain, fewer edge cases to handle, fewer bugs from unexpected option interactions.

A concrete example: [hollow chunk elimination](./app-scenario-chunking.md#hollow-chunk-elimination). With `strictExecutionOrder: true`, execution order is preserved via init calls, so removing a hollow chunk is straightforward — inline the init sequence at the call site. Without `strictExecutionOrder`, eliminating a hollow chunk while preserving execution order is a fundamentally harder problem — the bundler would need an entirely different mechanism to guarantee modules execute in the right order after the chunk is gone. Supporting both paths is costly and not worth it. With app scenario, we only implement the `strictExecutionOrder: true` path.

App scenario takes the opposite approach from general-purpose. Instead of trying to make every combination of options work, it fixes a known-good set of options and builds optimizations that assume all of them. This lets the chunking algorithm do things that are impossible in the general case — not because the individual features are new, but because **knowing they're all enabled together** unlocks compound optimizations that isolated features can't achieve.

General-purpose mode remains for libraries and other non-app outputs where these assumptions don't hold.

## Forced settings

| Option                    | Forced value | Why                                             |
| ------------------------- | ------------ | ----------------------------------------------- |
| `strictExecutionOrder`    | `true`       | Decouples module placement from execution order |
| `preserveEntrySignatures` | `false`      | Allows moving modules into/out of entry chunks  |

**Not supported:** Top Level Await (TLA). TLA introduces async initialization semantics incompatible with the synchronous init call model that app scenario relies on. May be considered in the future if needed.

### `strictExecutionOrder: true`

Without it, module placement IS execution order. Moving a module to a different chunk (or a different position within a chunk) can change when its top-level code runs, breaking side-effect-dependent code. The bundler is forced to be conservative.

With it, modules are wrapped in `init_xxx()` functions. Execution order is controlled by the init call graph, not by placement. Modules can go anywhere. See [strict-execution-order.md](./strict-execution-order.md).

### `preserveEntrySignatures: false`

By default, entry chunks must re-export everything the original entry module exports. This constrains what can be moved: pulling a module out of an entry creates a facade re-export layer.

With `false`, entries don't need to maintain their original export interface:

- Modules can be freely moved into entry chunks from other chunks
- Entry chunks can absorb shared modules without creating re-export facades
- The entry chunk becomes just another chunk that happens to be the load trigger

Together, these two settings make **entries just chunks** — the entry/non-entry distinction is only about which chunk the browser fetches first, not about module signatures or execution semantics.

## Chunking — the reason app scenario exists

App scenario exists because of chunking. The forced settings above aren't goals in themselves — they exist to remove constraints that prevent the chunking algorithm from doing what it should: optimize purely for loading performance, free from execution order and signature constraints.

See [app-scenario-chunking.md](./app-scenario-chunking.md) for the full chunking design.

## Related

- [app-scenario-chunking.md](./app-scenario-chunking.md) — chunking algorithm and capabilities
- [strict-execution-order.md](./strict-execution-order.md) — wrapping mechanism and init call optimization
- [manual-code-splitting](../../docs/in-depth/manual-code-splitting.md)
- [automatic-code-splitting](../../docs/in-depth/automatic-code-splitting.md)
