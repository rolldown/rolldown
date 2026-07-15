# Module Execution Order

## Summary

`CanonicalizeEntriesPass` and `ComputeModuleExecutionOrderPass` produce a deterministic, dependency-respecting execution order over every reachable module in the graph. The first pass consumes raw scan entries and returns the only owned `EntryPlanDraft`; the second borrows that draft and the module table, returns sealed `ModuleExecutionOrders` plus owned `SortedModules`, and does not mutate either input. The link driver temporarily projects assigned orders into `Module::exec_order` and moves the sorted list into the legacy carrier for downstream stages. It also retains the sealed typed order table through `BindImportsPass`, which uses it only to choose deterministic facade names for grouped external default imports. Chunk assembly, cross-chunk link computation, and rendering still rely on the legacy fields as the canonical "what runs before what" signal while the migration is in progress.

Sources: `crates/rolldown/src/stages/link_stage/passes/canonicalize_entries.rs` and `crates/rolldown/src/stages/link_stage/passes/compute_module_execution_order.rs`.

## Guarantees

The order is defined by a small set of rules, in precedence order:

1. **Runtime module is always first.** `sorted_modules[0] == runtime.id()` is asserted at the end of the pass. Generated helpers (`__commonJS`, `__toESM`, etc.) must be defined before any module that references them.
2. **User-defined entries execute in declaration order.** `CanonicalizeEntriesPass` preserves the order from `options.input`. It sorts only the dynamic-import and emitted-entry suffix by `(entry.kind, module.id().as_str())`, then groups entries by first root occurrence without creating a second root representation.
3. **Dependencies execute before dependents, along acyclic edges.** For any non-back edge `A → B` that the execution-order pass traverses, `orders[B] < orders[A]`. Back edges in cycles are the exception: when the algorithm revisits an already-executed ancestor it is skipped, so a module on a cycle can receive its order before a transitive dependency further along the cycle. Downstream stages must not assume strict topological order across cycles.
4. **`require(...)` is treated as a static import.** Because ES `import` statements are hoisted, required modules are placed after static imports. Among `require` calls, the first one encountered during AST scan wins — as the doc comment shows, for:
   ```js
   () => require('b');
   require('c');
   import 'a';
   ```
   the execution order is `a → b → c`.
5. **Dynamic imports are skipped by default.** They participate in sorting only when `code_splitting` is disabled (inline dynamic imports mode); otherwise they become separate chunk roots and their subgraphs are walked from their own entry status.
6. **Order is relative, not global.** The pass only guarantees that dependencies precede their dependents along traversed edges. It does not claim a canonical topological order across unrelated subtrees — the order is a function of the DFS traversal rooted at canonical entries.

## Algorithm

The pass is an **iterative post-order DFS** with an explicit `execution_stack: Vec<Status>`. Using an explicit stack (rather than recursion) avoids stack overflow on deep module graphs, which are common in real-world codebases.

### Two-state stack entries

```rust
enum Status {
  ToBeExecuted(ModuleIdx),  // pre-visit: needs its deps pushed
  WaitForExit(ModuleIdx),   // post-visit: deps are done, assign exec_order
}
```

On its first real visit, a `ModuleIdx` goes through the `ToBeExecuted → WaitForExit` pair: popped as `ToBeExecuted` (pre-order), its own `WaitForExit` (post-order sentinel) is pushed, then its dependencies are pushed above it. When the sentinel is popped, every transitive dependency has already been assigned a lower `exec_order` — the iterative equivalent of "assign order after returning from the recursive call." Later incoming edges may push additional `ToBeExecuted(id)` entries for the same module (e.g. the second importer in a diamond); these are popped and short-circuited by the `executed_ids` membership check rather than re-entering the pre/post pair. The complexity section below spells out the consequences for the total push count.

### Seeding the stack

```rust
let mut execution_stack = entry_plan
  .roots()
  .rev()
  .map(Status::ToBeExecuted)
  .chain(iter::once(Status::ToBeExecuted(runtime)))
  .collect();
```

Entries are pushed **in reverse**, then the runtime is pushed **last** — because a `Vec` stack pops from the end, this makes the runtime pop first and entries pop in original declaration order. That single `.rev() + chain` pair is what pins down rules (1) and (2) above.

### Visiting dependencies

On `ToBeExecuted(id)`:

- If `id` is already in `executed_ids`, it is skipped (may trigger a cycle diagnostic; see below).
- Otherwise, `id` is inserted into `executed_ids`, a `WaitForExit(id)` sentinel is pushed, and the module's import records are filtered and pushed in reverse:
  ```rust
  rec.kind.is_static()
    || (code_splitting_disabled && rec.kind.is_dynamic())
  ```
  `.rev()` preserves the source-order of imports after the stack reverses them.

On `WaitForExit(id)`: assign `execution_orders[id] = next_exec_order`, push `id` onto the owned sorted list if it is a `Module::Normal`, increment the counter, and remove the stack-index bookkeeping entry. External modules receive an order but do not appear in `SortedModules` because only normal modules are emitted by the chunk pipeline. The pass leaves every legacy `module.exec_order` untouched; the driver is the only temporary compatibility writer.

### Circular-dependency detection

Cycles are detected opportunistically and only reported when the user opts in via `options.checks.contains(EventKindSwitcher::CircularDependency)`.

The key bookkeeping is `stack_indexes_of_executing_id: FxHashMap<ModuleIdx, usize>`, which records the position of each module's `WaitForExit` sentinel while it is live on the stack. When `ToBeExecuted(id)` is popped for an already-executed `id`:

- If `stack_indexes_of_executing_id` still has an entry for `id`, the module is a **back edge** — we're currently in the middle of processing it deeper in the DFS. The cycle is recovered by slicing the stack from that index to the top and collecting every `WaitForExit` variant along the way (those are the modules on the active DFS chain).
- If `id` is executed but not in the map, it's a **cross edge** — already finished, no cycle.

Cycles are deduplicated via `FxIndexSet<Box<[ModuleIdx]>>` and emitted as `BuildDiagnostic::circular_dependency` warnings (not errors) at the end of the pass. The indexed set preserves first DFS-discovery order. This intentionally removes the old cross-process warning-order variation from iterating an `FxHashSet`; it does not change cycle discovery, path construction, or deduplication.

### Complexity

Each module has exactly one real visit — one `WaitForExit` push and one `exec_order` assignment. `ToBeExecuted` pushes, by contrast, are per-edge: the module is pushed once per incoming edge (static import, retained dynamic import when code splitting is disabled, or initial seed from the entry list / runtime), and any push past the first is popped and skipped cheaply via the `executed_ids` membership check. In a diamond `A → C, B → C`, C gets two `ToBeExecuted` pushes and one `WaitForExit` push.

Overall work is O(N) real visits plus O(E) dependency pushes-and-skips, so total O(N + E), with constant-time hash set membership. `FxHashSet::with_capacity(module_count)` avoids rehashing on `executed_ids`.

## Downstream Consumers

`exec_order` and `sorted_modules` are consumed in several places:

| Consumer                                         | Use                                                                                                                                          |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `generate_stage/code_splitting.rs`               | Iterates `sorted_modules` to assign modules to chunks; compares `exec_order` to order entry-vs-common chunks deterministically               |
| `chunk_graph.rs`                                 | Sorts modules within a chunk by `exec_order` (also used as tiebreaker after side-effect-free leaf grouping)                                  |
| `generate_stage/compute_cross_chunk_links.rs`    | Orders cross-chunk imports for stable output                                                                                                 |
| `generate_stage/manual_code_splitting.rs`        | Uses exec order as a stable seed for user-defined chunk groupings                                                                            |
| `ecmascript/ecma_generator.rs`                   | Carries `exec_order` through to render so output module sequences stay deterministic                                                         |
| `stages/link_stage/cross_module_optimization.rs` | Walks `sorted_modules` for deterministic iteration over the graph                                                                            |
| `stages/link_stage/passes/bind_imports.rs`       | Reads sealed `ModuleExecutionOrders` to choose the stable minimum `(execution order, symbol name)` for external default-import facade naming |

Because so many later stages key off `exec_order`, any change to the traversal rules here is an observable output change across the entire bundler. The typed execution-order chain therefore runs before TLA and all remaining link analyses, and the driver performs its compatibility projection before any legacy reader. That projection does not consume `ModuleExecutionOrders`; the sealed table remains live until binding returns, then leaves scope before resolved-export finalization.

## Invariants

- `module.exec_order == u32::MAX` and `execution_orders[module] == u32::MAX` before each artifact assignment. The pass does not mutate the module table; the driver separately asserts the legacy slot is still `u32::MAX` before projection.
- `sorted_modules.first() == Some(runtime.id())` (asserted at the end of the pass).
- A module may appear in `execution_stack` under `Status::WaitForExit` at most once concurrently; `stack_indexes_of_executing_id.contains_key(&id)` is asserted to be false before insertion.
- `sorted_modules` contains every `Module::Normal` reachable from the entries or runtime. External modules get an `exec_order` but are not pushed into `sorted_modules`.
- `EntryPlanDraft` has no public constructor or clone path. It is produced once, borrowed by execution ordering, and then consumed into the legacy entries map.
- `ModuleExecutionOrders` is available only as `Sealed<ModuleExecutionOrders>` and remains live through `BindImportsPass`; `SortedModules` moves without cloning into the legacy carrier.

## Unresolved Questions

- **Cycle reporting granularity.** Only one cycle per "entry point into the SCC" is reported — cycles sharing a back edge through different paths may be collapsed. If users want complete SCC reporting, Tarjan's algorithm would be the right tool, but it's heavier and rarely needed.

## Related

- [code-splitting](../../code-splitting/implementation.md) — consumes `exec_order` for chunk assembly
- [runtime-helpers](../../runtime-helpers/implementation.md) — the runtime module whose "always first" guarantee this pass provides
- `crates/rolldown/src/stages/link_stage/passes/canonicalize_entries.rs` — entry producer
- `crates/rolldown/src/stages/link_stage/passes/compute_module_execution_order.rs` — execution-order producer
- `crates/rolldown/src/stages/link_stage/mod.rs` — `LinkStage::link` pipeline ordering
