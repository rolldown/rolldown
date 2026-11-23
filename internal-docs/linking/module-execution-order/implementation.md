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

The link-time constant lookup in `refine_stmt_side_effects.rs` therefore rejects imported constants within the same static-import strongly connected component, even if the exporter has a lower `exec_order`. With multiple entries or preserved modules, loading a different entry can reverse the order within the cycle and expose a `const` TDZ failure. These components are computed lazily once and shared with the analyzer reruns in cross-module optimization. An exporter outside the importer's component can still supply a constant when it precedes the importer.

The entry re-export fallback in `esm_init_obligations::collect_entry_reexported_wrapper_inits` likewise applies only to unwrapped entries. An ESM-wrapped entry initializes its static dependencies through its own wrapper in source order, including across chunks. Calling its re-export owners first in `exec_order` can enter a dependency cycle through a different module and read an uninitialized binding (issue #11231).

Cross-module optimization checks binding initialization before it removes a call to a function that `export { fn }` or `export default fn` exports (`SymbolRefFlags::ExportedSeparately`). A function declaration is initialized before the module evaluates. An initializer for a local variable must precede the statement that reads it. For an imported variable, the exporter must precede the caller and the caller must first finish all static imports.

The caller can start before a static import finishes if it is in a cycle. This applies even when it reads a binding from an exporter outside its own component. For example, `main.js` can import `a.js` and re-export `fn` from `b.js`. If `a.js` imports `fn` from `main.js`, loading `main.js` can evaluate `a.js` before `b.js`. An external module can complete a cycle through an emitted module. The initialization check treats all static paths to an external module as unknown.

The linker checks binding reads in the entire statement, including arguments, before it relaxes the statement's side-effect flags. It also checks whether the result can run code, for example through coercion or destructuring. It keeps `while` and `do ... while` statements unless it can prove the test is false. The finalizer does not add a pure annotation for these calls, since the minifier does not have the linker's proof.

Cross-module optimization also looks for a top-level call that runs before the binding of its side-effect-free callee is initialized (`call_runs_before_init`). A top-level call is a call outside all functions and classes. A function declaration is initialized before any code runs, so a call to it does not run early. In the same module, an early call comes before the declaration of the binding. For an imported binding, every entry must run the caller and the owner, the module that declares the binding, in `exec_order`, and the owner must have a higher `exec_order` than the caller. Then the call runs before the owner evaluates. This is possible only in an import cycle.

To run the two modules in `exec_order`, both modules must be unwrapped, and the build must have no manual code splitting groups. With one entry, the chunks run their unwrapped modules in `exec_order`. With more entries, both modules must be in one region that code can start at only one module (`ImportCycles::runs_in_one_order`). The entry of an `import()` counts as an entry, so a region test is necessary also for a build with one input.

The caller must also not wait for an `await` (`ImportCycles::imports_may_await`). The caller, and each module that it reaches through static imports, must have no top-level `await` and must not be external. While the caller waits, Node runs other modules, for example the owner. The output runs the owner after the caller. An external module can have a top-level `await`. This is also true for a Node builtin, because `output.paths` can map it to another module. The linker sets `is_tla_or_contains_tla_dependency` only in `compute_tla`, after `include_statements`. So the check reads the `await` flags of the module and the static import graph. Node decides from the source that a module is async, also when its `await` never runs, such as `if (false) await 0;`. The dead code elimination before the scan, and `dropLabels`, can remove such an `await`, so the scanner flag `TopLevelAwait` does not see it. `PreProcessEcmaAst` looks for a top-level `await` before these steps and sets `SourceTopLevelAwait`. It walks the AST of each module where one of these steps runs. It does not search the source text, because `define` and the `transform_ast` hook of a plugin can insert an `await` that the text does not have. The check reads both flags. The scanner flag stays as it is, because it also gives the error for top-level `await` in non-ESM output, and `compute_tla` reads it. The fixtures are `tree_shaking/call_before_init_after_await`, `tree_shaking/call_before_init_await_in_same_statement`, `tree_shaking/call_before_init_await_in_dependency`, `tree_shaking/call_before_init_dead_await_in_dependency`, `tree_shaking/call_before_init_define_await_in_dependency`, the Rust test `plugin/transform_ast_dead_await` and `tree_shaking/call_before_init_await_in_external`.

A region is a component and the components that only it imports, directly or through other such components. A component with an entry starts its own region. Code can reach a region only through its first component. So when code can start that component at only one module, every entry runs the region in one order. Without this, two entries can start one cycle at different modules, and the chunk that holds the cycle still runs it in one order. The linker sees the modules of a `strictExecutionOrder` build as unwrapped, because the generate stage wraps them later. That output runs the modules in the source order of each entry.

The linker keeps an early call for both export forms. For a function that `export { fn }` exports, the initialization check above already keeps it. For a function that its declaration exports, the linker removes every other call. After `include_statements`, `protect_calls_before_init` protects each early call in an included statement, for every use of the result. The minifier can remove a pure call. It can also remove a call to another pure function and keep only the arguments that have side effects, so `noop(fn())` becomes `fn()`. The finalizer does not mark a protected call pure. When the chunk minifier runs and the output does not preserve modules, the finalizer also removes the `@__NO_SIDE_EFFECTS__` annotation of the callee (`SymbolRefFlags::MaybeCalledBeforeInit`), because the minifier treats every call of an annotated binding in the chunk as pure. The minifier treats `new` of an annotated binding as pure too, so the finalizer marks such a `new` in the chunk of the callee pure itself. Without the chunk minifier, or with `preserveModules`, the callee keeps its annotation for other bundlers.

A call in the same module gets a similar check before the linker runs. oxc treats every call of an annotated binding as pure, with no TDZ check. So before the per-module dead code elimination, `PreProcessEcmaAst` removes the `@__NO_SIDE_EFFECTS__` annotation of a `let`, `const` or `var` binding that code reads before its declaration, outside a nested function. `export { fn }` does not read the binding.

The check runs after `define` and before the transform. `define` can insert a read of a binding, and it can insert an annotated function. "Before" is AST order, not span order, because `define` gives an inserted read an empty span at 0. The check counts only a read that runs each time its statement runs, for the reason in the limit about conditions below. When the condition is a literal, the check knows the branch: it skips a branch that never runs, such as `if (false) fn();`, `false && fn()` or `0 ?? fn()`, and it counts a read in a branch that always runs, such as `true && fn()`. A declaration in a conditional part still counts as reached. Dead code that is not a branch still counts, for example code after a `throw`. Then the annotation goes and the output keeps the later calls, which costs size only. A call or a `new` under a condition before the declaration gets its own pure annotation, so that oxc still removes it when no code uses its result. The fixtures are `tree_shaking/call_before_init`, `tree_shaking/call_before_init_dead_branch` and `tree_shaking/call_before_init_define_read`.

Some early calls stay out of reach:

- oxc keeps a summary of each function whose body has no side effects, with or without the annotation. A later pass of its dead code elimination uses the summary to remove a call that comes before the declaration. This applies to the per-module pass and to the chunk minifier, also with `minify: 'dce-only'`. The first pass makes the summary when it gets to the declaration, so such a call stays only when the first pass changes nothing. The oxc minifier documents this as an assumption: code does not violate the TDZ.
- The check does not include a call that runs only under a condition: in a branch of `if` or `?:`, in the right operand of `&&`, `||` or `??`, in a logical assignment, in a loop body, in a `switch` case, in a `catch` clause, in a labeled statement, in an optional chain or in a default value. A condition can read state that another module sets, and the output can run the modules in another order than Node. For example, a shared chunk runs before an earlier import of the entry, and with `preserveModules` a module imports the owner of a re-exported binding before the barrel. Then the condition can be true in the output and false in Node, and a kept call would throw where Node does not. The test of an `if` or a loop, the left operand of `&&`, `||` and `??`, and a `finally` block run each time, so a call there counts. In a `try` block with a `catch` clause, only the calls that run first count, such as `f()` in `try { f(); g(); }` or in `try { noop(f()); }`. Code before another call can throw, and the `catch` clause can recover from that error. A call in the first argument of another call counts only when the read of the outer callee cannot throw. The read of an uninitialized binding throws, and a global can be a getter that throws. Then the argument runs in the output and not in Node, for example in `try { gate((f(), 0)); }` when the output runs the owner of `gate` first. The same-module check accepts only a function declaration of the module as the outer callee, because it runs before the linker can see imports. The linker check uses `binding_read_is_safe` on the canonical callee. In a module that is part of an import cycle, an imported callee passes this test only when a function declaration declares it. In a CommonJS module, the same-module check also does not count a read after a top-level `return`. The linker check does not need this rule: it sees only exported functions, and it needs unwrapped modules. The removed calls can make the first pass of the chunk minifier change the code, and then the oxc limit above can remove an early call of an empty function in the same chunk. The fixtures are `tree_shaking/call_before_init_guarded_dynamic_import`, `tree_shaking/call_before_init_guarded_barrel`, `tree_shaking/call_before_init_global_callee` and `tree_shaking/call_before_init_imported_callee`.
- The check does not include a call inside a function or a class, also a synchronous IIFE or a static class field that runs while the module evaluates. Such a call has the behavior of other calls. Another module can call a function at any time, so a check of these calls must know when the function runs. With `preserveModules`, the output can also start a cycle at another module than the source.
- An external module can import a module of the output and complete a cycle that the linker cannot see. The check does not look for that cycle. Most library modules import a package, so a check that keeps these calls would remove annotations in most library builds.
- Cross-module optimization visits the expression of `export default` only when the module has an `import()` or `optimization.inlineConst.pass` is higher than 1. Otherwise the check does not see an early call in `export default fn();`. The finalizer marks that call pure, so the minifier removes it when no code reads the default export.
- When the caller can wait for an `await`, the check does not include the call. This also applies to a call before the first `await`, such as `fn(); await 0;`, and to a caller whose only `await` never runs, so Node throws and the output does not. Any external module that the caller reaches turns off the check, also a Node builtin. Most library modules reach a package, so the check does not include most calls in library builds.
- When the entries can run the two modules in different orders, the check does not include the call. For example, two entries can start one cycle at different modules. Node throws when one of the entries loads, but the output does not throw.
- With `preserveModules`, the `exec_order` path expects code to load the output only through its entries. A deep import, or an external module that imports a module that is not an entry, can start a cycle at another module.
- With `preserveModules`, a module of the output imports the owner of a re-exported binding before the barrel, so the output can start a cycle at another module than the source. Rollup has the same order. The check keeps only calls that throw in the source, so this order does not add an error.

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

- [code-splitting](../../code-splitting/implementation.md) — consumes `exec_order` for chunk assembly
- [runtime-helpers](../../runtime-helpers/implementation.md) — the runtime module whose "always first" guarantee this pass provides
- `crates/rolldown/src/stages/link_stage/sort_modules.rs` — implementation
- `crates/rolldown/src/stages/link_stage/mod.rs` — `LinkStage::link` pipeline ordering
