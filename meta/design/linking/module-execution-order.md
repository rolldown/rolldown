# Module Execution Order

## Summary

`sort_modules` produces a deterministic, dependency-respecting execution order over every module in the graph. It runs as the first step of the link stage, assigns each module a monotonically increasing `exec_order: u32`, and writes the resulting module index sequence to `LinkStageOutput::sorted_modules`. Downstream stages (chunk assembly, cross-chunk link computation, render) rely on this order as the canonical "what runs before what" signal — it is the bridge between the resolved graph and every ordering decision the bundler makes.

Source: `crates/rolldown/src/stages/link_stage/sort_modules.rs`.

## Guarantees

The order is defined by a small set of rules, in precedence order:

1. **Runtime module is always first.** `sorted_modules[0] == runtime.id()` is asserted at the end of the pass. Generated helpers (`__commonJS`, `__toESM`, etc.) must be defined before any module that references them.
2. **User-defined entries execute in declaration order.** The order in which entries appear in `options.input` is preserved. Non-user entries (dynamic-import and emitted entries) are canonicalized earlier, in `LinkStage::new` (`crates/rolldown/src/stages/link_stage/mod.rs:142`), by sorting on `(item.kind, module.id().as_str())`. That sorted suffix is what this pass consumes — `sort_modules` itself does no entry reordering. (The `Module#debug_id` wording in the `sort_modules` source-code doc comment is stale; the authoritative key is the module id string.)
3. **Dependencies execute before dependents, along acyclic edges.** For any non-back edge `A → B` that `sort_modules` traverses, `B.exec_order < A.exec_order`. Back edges in cycles are the exception: when the algorithm revisits an already-executed ancestor it is skipped (that's where cycle detection fires), so a module on a cycle can receive its `exec_order` before a transitive dependency further along the cycle. Downstream stages must not assume strict topological order across cycles.
4. **`require(...)` is treated as a static import.** Because ES `import` statements are hoisted, required modules are placed after static imports. Among `require` calls, the first one encountered during AST scan wins — as the doc comment shows, for:
   ```js
   () => require('b');
   require('c');
   import 'a';
   ```
   the execution order is `a → b → c`.
5. **Dynamic imports are skipped by default.** They participate in sorting only when `code_splitting` is disabled (inline dynamic imports mode); otherwise they become separate chunk roots and their subgraphs are walked from their own entry status.
6. **Order is relative, not global.** `sort_modules` only guarantees that dependencies precede their dependents along traversed edges. It does not claim a canonical topological order across unrelated subtrees — the order is a function of the DFS traversal rooted at entries.

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
let mut execution_stack = self
  .entries
  .keys()
  .rev()
  .map(|&idx| Status::ToBeExecuted(idx))
  .chain(iter::once(Status::ToBeExecuted(self.runtime.id())))
  .collect();
```

Entries are pushed **in reverse**, then the runtime is pushed **last** — because a `Vec` stack pops from the end, this makes the runtime pop first and entries pop in original declaration order. That single `.rev() + chain` pair is what pins down rules (1) and (2) above.

### Visiting dependencies

On `ToBeExecuted(id)`:

- If `id` is already in `executed_ids`, it is skipped (may trigger a cycle diagnostic; see below).
- Otherwise, `id` is inserted into `executed_ids`, a `WaitForExit(id)` sentinel is pushed, and the module's import records are filtered and pushed in reverse:
  ```rust
  rec.kind.is_static()
    || (self.options.code_splitting.is_disabled() && rec.kind.is_dynamic())
  ```
  `.rev()` preserves the source-order of imports after the stack reverses them.

On `WaitForExit(id)`: assign `module.exec_order = next_exec_order`, push onto `sorted_modules` if it's a `Module::Normal`, increment the counter, and remove the stack-index bookkeeping entry. External modules get an `exec_order` but do not appear in `sorted_modules` (only normal modules are emitted by the chunk pipeline).

### Circular-dependency detection

Cycles are detected opportunistically and only reported when the user opts in via `options.checks.contains(EventKindSwitcher::CircularDependency)`.

The key bookkeeping is `stack_indexes_of_executing_id: FxHashMap<ModuleIdx, usize>`, which records the position of each module's `WaitForExit` sentinel while it is live on the stack. When `ToBeExecuted(id)` is popped for an already-executed `id`:

- If `stack_indexes_of_executing_id` still has an entry for `id`, the module is a **back edge** — we're currently in the middle of processing it deeper in the DFS. The cycle is recovered by slicing the stack from that index to the top and collecting every `WaitForExit` variant along the way (those are the modules on the active DFS chain).
- If `id` is executed but not in the map, it's a **cross edge** — already finished, no cycle.

Cycles are deduplicated via `FxHashSet<Box<[ModuleIdx]>>` and emitted as `BuildDiagnostic::circular_dependency` warnings (not errors) at the end of the pass.

### Complexity

Each module has exactly one real visit — one `WaitForExit` push and one `exec_order` assignment. `ToBeExecuted` pushes, by contrast, are per-edge: the module is pushed once per incoming edge (static import, retained dynamic import when code splitting is disabled, or initial seed from the entry list / runtime), and any push past the first is popped and skipped cheaply via the `executed_ids` membership check. In a diamond `A → C, B → C`, C gets two `ToBeExecuted` pushes and one `WaitForExit` push.

Overall work is O(N) real visits plus O(E) dependency pushes-and-skips, so total O(N + E), with constant-time hash set membership. `FxHashSet::with_capacity(module_count)` avoids rehashing on `executed_ids`.

## Downstream Consumers

`exec_order` and `sorted_modules` are consumed in several places:

| Consumer                                         | Use                                                                                                                            |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `generate_stage/code_splitting.rs`               | Iterates `sorted_modules` to assign modules to chunks; compares `exec_order` to order entry-vs-common chunks deterministically |
| `chunk_graph.rs`                                 | Sorts modules within a chunk by `exec_order` (also used as tiebreaker after side-effect-free leaf grouping)                    |
| `generate_stage/compute_cross_chunk_links.rs`    | Orders cross-chunk imports for stable output                                                                                   |
| `generate_stage/manual_code_splitting.rs`        | Uses exec order as a stable seed for user-defined chunk groupings                                                              |
| `ecmascript/ecma_generator.rs`                   | Carries `exec_order` through to render so output module sequences stay deterministic                                           |
| `stages/link_stage/cross_module_optimization.rs` | Walks `sorted_modules` for deterministic iteration over the graph                                                              |

Because so many later stages key off `exec_order`, any change to the traversal rules here is an observable output change across the entire bundler. That's also why `sort_modules` is the very first step in `LinkStage::link` — it must happen before any stage that reads the field.

## Invariants

- `module.exec_order == u32::MAX` before the pass (asserted via `debug_assert!` on assignment). This catches double-sort and bypassed traversal.
- `sorted_modules.first() == Some(runtime.id())` (asserted at the end of the pass).
- A module may appear in `execution_stack` under `Status::WaitForExit` at most once concurrently; `stack_indexes_of_executing_id.contains_key(&id)` is asserted to be false before insertion.
- `sorted_modules` contains every `Module::Normal` reachable from the entries or runtime. External modules get an `exec_order` but are not pushed into `sorted_modules`.

## Unresolved Questions

- **Cycle reporting granularity.** Only one cycle per "entry point into the SCC" is reported — cycles sharing a back edge through different paths may be collapsed. If users want complete SCC reporting, Tarjan's algorithm would be the right tool, but it's heavier and rarely needed.

## Related

- [code-splitting](../code-splitting.md) — consumes `exec_order` for chunk assembly
- [runtime-helpers](../runtime-helpers.md) — the runtime module whose "always first" guarantee this pass provides
- `crates/rolldown/src/stages/link_stage/sort_modules.rs` — implementation
- `crates/rolldown/src/stages/link_stage/mod.rs` — `LinkStage::link` pipeline ordering
