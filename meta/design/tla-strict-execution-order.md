# Full TLA Semantics under strictExecutionOrder

## Summary

When `strictExecutionOrder` is enabled, rolldown wraps every module in a lazy init function (`__esm`/`__esmMin`). For modules with top-level await (TLA) or transitive TLA dependencies, the wrapper is made `async` and call sites use `await init_X()`. This design has two bugs:

1. **Deadlock** ([#9548](https://github.com/rolldown/rolldown/issues/9548)): circular dependencies with TLA create an unresolvable async init cycle.
2. **Sequential loading**: `await init_A(); await init_B()` prevents independent async dependencies from running concurrently, violating the spec's parallel evaluation model.

This document analyzes the ECMAScript spec's TLA evaluation algorithm, compares rolldown's current implementation with webpack's approach, and proposes a fix.

## Background: the spec's module evaluation algorithm

References: [ECMA-262 §16.2.1.5.2 InnerModuleEvaluation](https://tc39.es/ecma262/#sec-innermoduleevaluation), [§16.2.1.5.2.3 AsyncModuleExecutionFulfilled](https://tc39.es/ecma262/#sec-async-module-execution-fulfilled).

The spec evaluates modules via a DFS traversal (`InnerModuleEvaluation`) with the following key mechanisms:

**Module states**: `linked` → `evaluating` → `evaluating-async` → `evaluated`

**Key fields per module**:

- `[[HasTLA]]`: whether the module's own source contains `await` (static, set at parse time)
- `[[AsyncEvaluationOrder]]`: `~unset~` initially; set to an incrementing integer when the module is on the async path (has TLA or has async deps)
- `[[PendingAsyncDependencies]]`: counter of async deps that haven't completed yet
- `[[AsyncParentModules]]`: list of modules waiting for this module to complete

**Algorithm outline**:

```
InnerModuleEvaluation(module):
  step 2: if module.status is evaluating-async or evaluated → return (already processed)
  step 3: if module.status is evaluating → return (cycle back-edge, NO-OP)
  step 5: module.status = evaluating
  step 8: module.pendingAsyncDeps = 0
  step 11: for each dependency:
    InnerModuleEvaluation(dep)
    step 11.c.v: if dep.asyncEvaluationOrder is an integer:
      module.pendingAsyncDeps++
      dep.asyncParentModules.push(module)
  step 12: if module.pendingAsyncDeps > 0 or module.hasTLA:
    module.asyncEvaluationOrder = next_counter()
    if module.pendingAsyncDeps == 0: ExecuteAsyncModule(module)  // start body
    else: (defer, wait for deps)
  step 13: else: ExecuteModule(module)  // sync, execute immediately
  step 16: (Tarjan SCC finalization)
```

**Critical detail about step 11.c.v**: this check runs for ALL cyclic module deps, including those still in `evaluating` state (same SCC). The cycle safety comes from the fact that the back-edge target hasn't reached step 12 yet, so its `asyncEvaluationOrder` is still `~unset~` — the check doesn't fire, no `pendingAsyncDeps` edge is created. Forward edges within the SCC (where the dep has already passed step 12) DO create edges.

**When an async module completes** (`AsyncModuleExecutionFulfilled`): decrements `pendingAsyncDeps` of each parent in `asyncParentModules`. Parents whose counter hits 0 are collected into `execList`, sorted by `asyncEvaluationOrder`, and executed in that order. Non-TLA parents execute synchronously; TLA parents start via `ExecuteAsyncModule`.

**Key properties**:

- Independent async deps run concurrently (both started during DFS, both suspended at `await`, resolved independently)
- A module doesn't execute until ALL its async deps complete (implicit `Promise.all` via the counter)
- Within-SCC cycle back-edges are no-ops (no deadlock)
- Execution order of simultaneously-ready modules is deterministic (sorted by `asyncEvaluationOrder` = DFS post-order)

## Current rolldown implementation

The `__esmMin` runtime helper:

```js
var __esmMin = (fn, res) => () => (fn && (res = fn((fn = 0))), res);
```

Two states: `fn` is the function (not called) or `0` (called). On re-entry, returns `res`.

For TLA-affected modules (`is_tla_or_contains_tla_dependency = true`), the wrapper is made `async` and call sites use `await`:

```js
var init_foo = __esmMin(async () => {
  await init_bar();
  // module body
});
```

### Bug 1: deadlock in cycles

When a cycle exists in the async init graph, re-entry returns `res` — the pending Promise from the first call. The caller `await`s it, creating a circular wait.

Example from [#9548](https://github.com/rolldown/rolldown/issues/9548): `init_public_app → init_internal_app → init_virtual_plugins → init_dev_server_logs → init_public_app` (re-entry returns pending Promise → deadlock).

The spec avoids this at step 3: when a module is re-entered during evaluation (`status === evaluating`), it returns immediately without blocking.

### Bug 2: sequential loading

```js
await init_A(); // A starts, A completes
await init_B(); // THEN B starts, B completes
```

B doesn't start until A finishes. If A and B have independent async deps (e.g., both `await fetch(...)`), native ESM would run them concurrently. Rolldown forces them to be sequential.

### Additional issue: `is_tla_or_contains_tla_dependency` is too coarse

This flag conflates two spec concepts:

- `[[HasTLA]]`: the module's own source contains `await`
- "has async dependency": the module transitively depends on a TLA module

Rolldown uses one flag for both, making ALL modules in the chain get `async` wrappers. The spec only marks modules with `[[HasTLA]]` as having async bodies; other modules are synchronous but deferred until their async deps complete.

## Webpack's approach

Webpack uses a fundamentally different runtime: a queue-based pub/sub system (`__webpack_require__.a`, ~70 lines).

**How it works**:

- Each async module's `module.exports` is replaced with a Promise before the body executes
- Real exports are stashed on the Promise via a Symbol (`webpackExports`)
- A queue (array with `.d` state flag: `-1` not started, `0` in progress, `1` resolved) tracks completion
- `__webpack_handle_async_dependencies__` collects async dep Promises, with a sync fast path when all deps are already resolved
- When a dep completes, its queue fires synchronous callbacks to decrement parent counters
- The `hasAwait` parameter (third arg) distinguishes modules with actual TLA from those that are only async due to deps

**Key differences from rolldown**:

| Aspect             | Webpack                                                           | Rolldown                                                    |
| ------------------ | ----------------------------------------------------------------- | ----------------------------------------------------------- |
| Module model       | Isolated scope per module, `__webpack_require__` mediates access  | Scope hoisting, shared scope, direct variable references    |
| Async waiting      | Queue + reference counting (synchronous callbacks)                | `await init_X()` (Promise chain)                            |
| Cycle handling     | Promise cached before body executes; queue unblocks on completion | `__esmMin` idempotent return; function declarations hoisted |
| Concurrent deps    | `__webpack_handle_async_dependencies__` starts all, waits for all | Sequential `await` (currently)                              |
| HasTLA distinction | `hasAwait` parameter controls queue creation                      | `is_tla_or_contains_tla_dependency` conflates both          |
| Runtime size       | ~70 lines                                                         | ~3 lines                                                    |

**Ordering correctness**: webpack's queue uses synchronous callbacks, so when multiple modules become ready from the same async completion, they execute in registration order (= DFS order = spec's `asyncEvaluationOrder`). This matches native ESM.

## Proposed fix: four-state `__esmMinAsync`

The core insight: the two-state model (`fn` truthy vs `0`) conflates three distinct spec states. A four-state model correctly maps to the spec:

| `fn` value             | Meaning                                                       | Spec equivalent    | Re-entry behavior                        |
| ---------------------- | ------------------------------------------------------------- | ------------------ | ---------------------------------------- |
| function (truthy)      | Not started                                                   | `linked`           | Execute                                  |
| `false`                | Synchronous execution in progress (before first `await`)      | `evaluating`       | → `void 0` (cycle, don't block)          |
| `0`                    | Async execution in progress (after function returned Promise) | `evaluating-async` | → return `res` (concurrent access, wait) |
| `0` + Promise resolved | Completed                                                     | `evaluated`        | → return `res` (resolved, instant)       |

```js
var __esmMinAsync = (fn, res) => () =>
  fn ? ((fn = ((res = fn((fn = false))), 0)), res) : fn === false ? void 0 : res;
```

**Execution trace**:

1. First call: `fn` is the function (truthy). Save `fn`, set `fn = false` (cycle guard). Call the function — during this synchronous execution, any re-entry sees `fn === false` → returns `void 0`. The function returns a Promise. Set `fn = 0` (transition to async-in-progress). Return the Promise.
2. Cycle re-entry during synchronous execution: `fn === false` → `void 0`. Matches spec step 3.
3. Concurrent access from a different branch: `fn === 0` → return `res` (the pending Promise). Caller awaits it, correctly waiting for completion.
4. After completion: `fn === 0`, `res` is a resolved Promise → return it, `await` resolves immediately.

**Why `false` vs `0` matters**: during the synchronous execution of the async function (before the first `await` returns), the function is on the call stack — any re-entry through a cycle back-edge happens HERE. After the function returns its Promise (`fn` transitions to `0`), it's no longer on the stack — any subsequent call is concurrent access from a different dependency branch and must wait.

The profiler-names variant:

```js
var __esmAsync = (fn, res) =>
  function () {
    return fn
      ? ((fn = ((res = (0, fn[__getOwnPropNames(fn)[0]])((fn = false))), 0)), res)
      : fn === false
        ? void 0
        : res;
  };
```

### Concurrent dependency loading

Change init call generation from:

```js
await init_A();
await init_B();
```

To:

```js
await Promise.all([init_A(), init_B()]);
```

This starts all deps before awaiting any, matching the spec's behavior where all deps are started during the DFS traversal and run concurrently.

### Constraint: always use `Promise.all`, even for single-dependency modules

When generating concurrent init calls, always wrap in `Promise.all([...])` — do NOT optimize single-dependency modules to `await init_X()` directly. This matters for execution ordering.

**Tested scenario**: A depends on C(TLA) and D(TLA). B depends on D only. Both C and D use `await Promise.resolve()`.

| B's codegen                     | Native ESM | Webpack | Four-state |
| ------------------------------- | ---------- | ------- | ---------- |
| `await Promise.all([init_D()])` | [A, B]     | [A, B]  | [A, B] ✓   |
| `await init_D()` (optimized)    | [A, B]     | [A, B]  | [B, A] ✗   |

**Cause of divergence with direct `await`**: `Promise.all` adds one microtask hop (internal promise resolution). When B uses `await init_D()` directly, B's `.then` handler is registered directly on D's Promise (one hop). A's handler goes through `Promise.all` (two hops). D resolves → B's handler fires first → B runs before A.

When both use `Promise.all`, both handlers go through the same number of hops, so registration order (= DFS order) is preserved. This matches the spec.

**When different-timing deps are used** (e.g., C=100ms, D=50ms), all approaches produce the same order [B, A] — determined by which dep completes first, not by microtask ordering.

**Remaining known deviations** (shared with webpack, inherent to user-space simulation):

- Extra microtask ticks from `async`/`await` compared to native ESM
- No `[[CycleRoot]]` concept — each wrapper holds its own Promise independently

## Implementation

### Runtime changes (`runtime-base.js`)

Add two new helpers:

```js
export var __esmAsync = (fn, res) =>
  function () {
    return fn
      ? ((fn = ((res = (0, fn[__getOwnPropNames(fn)[0]])((fn = false))), 0)), res)
      : fn === false
        ? void 0
        : res;
  };
export var __esmMinAsync = (fn, res) => () =>
  fn ? ((fn = ((res = fn((fn = false))), 0)), res) : fn === false ? void 0 : res;
```

**Constraint**: the callback passed to `__esmMinAsync`/`__esmAsync` MUST be an `async` function. If a non-async function throws synchronously, `fn((fn = false))` sets `fn` to `false` before the throw propagates, and the outer assignment `fn = (..., 0)` never executes — `fn` is stuck at `false` permanently (every subsequent call returns `void 0`). This cannot happen with `async` functions, which always return a Promise (synchronous throws become rejections). Rolldown already generates `async` wrappers for all TLA-affected modules, so this constraint is satisfied.

**Error propagation**: if the async init rejects (the module body throws), `res` holds the rejected Promise and `fn` transitions to `0`. All subsequent calls return the same rejected Promise — the caller's `await` re-throws the error. This matches the spec's behavior where a module in `evaluated` state with `[[EvaluationError]]` re-throws on every access.

### Runtime symbol registration

**`crates/rolldown_common/src/generated/runtime_helper.rs`** — this file is auto-generated by `tasks/generator/src/generators/runtime_helper.rs` from the runtime JS source. After adding the helpers to `runtime-base.js`, run the generator to update:

- `RuntimeHelper` bitflags: add `EsmAsync` and `EsmMinAsync` constants
- `RUNTIME_HELPER_NAMES` array: add `"__esmAsync"` and `"__esmMinAsync"`

No changes needed to `RuntimeModuleBrief::resolve_symbol()` or `validate_symbols` — they already work with any symbol name in the runtime module.

### Finalizer: wrapper selection

**`crates/rolldown/src/module_finalizers/impl_visit_mut.rs:259-264`** — the `visit_program` method, inside the `Some(WrapKind::Esm)` branch, currently selects between `__esm` and `__esmMin` based on `profiler_names`:

```rust
// Current:
let esm_ref = if self.ctx.options.profiler_names {
    self.canonical_ref_for_runtime("__esm")
} else {
    self.canonical_ref_for_runtime("__esmMin")
};

// Change to:
let is_async = self.ctx.linking_info.is_tla_or_contains_tla_dependency;
let esm_ref = match (self.ctx.options.profiler_names, is_async) {
    (true, true)   => self.canonical_ref_for_runtime("__esmAsync"),
    (true, false)  => self.canonical_ref_for_runtime("__esm"),
    (false, true)  => self.canonical_ref_for_runtime("__esmMinAsync"),
    (false, false) => self.canonical_ref_for_runtime("__esmMin"),
};
```

The `esm_wrapper_stmt` call at line 279 already receives `is_tla_or_contains_tla_dependency` as the `is_async` parameter — it makes the closure `async`. No change needed there.

### Finalizer: concurrent init calls (main work)

**`crates/rolldown/src/module_finalizers/mod.rs`** — five call sites generate init calls. For TLA modules, init calls need to be collected and emitted as a single `await Promise.all([...])` at the top of the wrapper body.

**Overall strategy**: when the CURRENT module has `is_tla_or_contains_tla_dependency`, during `remove_unused_top_level_stmt` (which runs BEFORE `walk_mut::walk_program`), collect **ALL** init call expressions (both sync and async) into a new `pending_init_calls` field on `ScopeHoistingFinalizer` and **remove** the original statement (don't push to `program.body`). After `walk_mut` and the `old_body` split, emit a single `await Promise.all([...])` prepended to `stmts_inside_closure`.

**Why ALL init calls, not just async**: `Promise.all` evaluates arguments left-to-right synchronously before awaiting. Sync init calls (`init_sync()`) complete immediately during argument evaluation, producing `undefined`. `Promise.all([undefined, pending_promise])` handles this correctly — `undefined` is wrapped in a resolved Promise. This preserves source-order initialization: sync deps initialize first (matching spec DFS order), then async deps start. If only async init calls were collected and sync ones stayed inline (after Promise.all), sync deps would initialize AFTER async deps complete — violating the spec's "sync deps first" ordering.

**Why hoisting is correct**: ESM semantics hoist all imports before body code. Moving init calls before interleaved body statements matches the spec more closely than the current inline placement.

Add `pending_init_calls: Vec<ast::Expression<'ast>>` as a new field on `ScopeHoistingFinalizer` (line 85-96 of `mod.rs`), initialized as empty in `ScopeHoistingFinalizerContext::finalize_normal_module` (line 77-93 of `finalizer_context.rs`). The collection is only active when the current module has `is_tla_or_contains_tla_dependency`. For modules without TLA deps, init calls continue to emit inline as before (no Promise.all overhead).

**Critical**: all five call sites must push **bare `CallExpression`s** into `pending_init_calls`, never `AwaitExpression`-wrapped calls. `Promise.all([await init_A(), init_B()])` evaluates `await init_A()` first (suspending before `init_B()` starts) — still sequential. The `await` belongs only on the outer `Promise.all`, not on individual elements.

**Invariant**: all five call sites are invoked exclusively from `remove_unused_top_level_stmt`, which runs before `walk_mut`. No init calls are generated during `walk_mut`, so `pending_init_calls` is stable by the time it is consumed for `Promise.all` emission.

#### Call site 1: `transform_or_remove_import_export_stmt` (line 316-453)

The `WrapKind::Esm` branch (line 406-449) handles each import statement individually. When the importee has `is_tla_or_contains_tla_dependency`, it currently generates `await init_X()` inline.

**Change**: when the CURRENT module has `is_tla_or_contains_tla_dependency`, collect ALL init calls (sync and async) into `self.pending_init_calls` and return `true` (remove statement). When the current module does NOT have TLA deps, keep current behavior (emit inline with `await` where applicable).

```rust
// Change (line 406-449, WrapKind::Esm branch):
WrapKind::Esm => {
    // ... (existing dedup check) ...

    let init_call = /* existing init call construction */;

    if self.ctx.linking_info.is_tla_or_contains_tla_dependency {
        // Current module is TLA-affected → collect for Promise.all
        self.pending_init_calls.push(init_call);
        return true;  // remove this statement
    } else if importee_linking_info.is_tla_or_contains_tla_dependency {
        // Current module is NOT TLA-affected but importee is → await inline (existing behavior)
        *stmt = /* await init_call */;
        return false;
    } else {
        *stmt = self.snippet.builder.statement_expression(SPAN, init_call);
        return false;
    }
}
```

#### Call site 2: `wrapped_esm_init_stmt_for_import_record` (line 244-312)

Generates init calls for import records where the importee's wrapper is in the same chunk. Lines 288-294 currently wrap individual calls in `AwaitExpression`, and lines 296-307 join multiple calls into a comma expression with sequential `await`s.

**Change**: when the current module has `is_tla_or_contains_tla_dependency`, collect ALL init call expressions into `self.pending_init_calls` and return `None`. When the current module does NOT have TLA deps, keep current behavior.

This also fixes the existing bug where multiple TLA init calls were joined into comma-separated sequential `await`s (`await init_a(), await init_b()`).

#### Call site 3: `generate_transitive_esm_init` (line 130-179)

Generates init calls for ESM-wrapped modules reached through excluded barrel re-exports. The method checks `WrapKind::Esm` (line 139) and generates plain `init_call` expression statements **without `await`** (lines 160-167).

**Change needed**: when the current module has `is_tla_or_contains_tla_dependency`, collect ALL transitive init calls into `self.pending_init_calls` instead of emitting inline. When the current module does NOT have TLA deps, keep current behavior (emit inline without await).

```rust
// Change (line 152-170):
if self.ctx.linking_info.is_tla_or_contains_tla_dependency {
    self.pending_init_calls.push(init_call);
} else {
    body.push(self.snippet.builder.statement_expression(SPAN, init_call));
}
```

#### Call site 4: `export * from` handler in `remove_unused_top_level_stmt`

**`mod.rs` lines 1630-1651** — when an `export * from './path'` is processed and the importee is ESM-wrapped, this code emits `init_X()` (with `await` if TLA) directly to `program.body`, bypassing `transform_or_remove_import_export_stmt`.

**Change needed**: when the current module has `is_tla_or_contains_tla_dependency`, collect the init call expression into `self.pending_init_calls` instead of pushing a statement to `program.body`.

#### Call site 5: cross-chunk init calls in `render_chunk_exports.rs`

Entry chunk rendering at line 24-37 emits a single `await init_entry()`. This does not need Promise.all because there is only one init call. No change needed.

#### Emitting the collected `Promise.all`

In `visit_program` (`impl_visit_mut.rs`), inside the `Some(WrapKind::Esm)` branch (line 185-287), after `walk_mut::walk_program` transforms all statements and populates `pending_init_calls`:

**Timing**: the insertion must happen AFTER `walk_mut` returns but BEFORE the `ConcatenateWrappedModuleKind` early returns and the `esm_wrapper_stmt` call. Specifically, insert right after the `old_body` split (line 192-210) and before the concatenated module checks (line 251).

```rust
// After old_body is split into fn_stmts + stmts_inside_closure:
if !self.pending_init_calls.is_empty() {
    let init_exprs = self.pending_init_calls.drain(..);
    let array_expr = self.snippet.builder.expression_array(
        SPAN,
        self.snippet.builder.vec_from_iter(
            init_exprs.map(|e| ast::ArrayExpressionElement::from(e))
        ),
        NONE,
    );
    let promise_all_call = self.snippet.builder.expression_call(
        SPAN,
        self.snippet.builder.expression_static_member(
            SPAN,
            self.snippet.builder.expression_identifier(SPAN, "Promise"),
            self.snippet.builder.identifier_name(SPAN, "all"),
            false,
        ),
        NONE,
        self.snippet.builder.vec1(ast::Argument::from(array_expr)),
        false,
    );
    let await_promise_all = self.snippet.builder.statement_expression(
        SPAN,
        ast::Expression::AwaitExpression(
            self.snippet.builder.alloc_await_expression(SPAN, promise_all_call),
        ),
    );
    stmts_inside_closure.insert(0, await_promise_all);
}
```

**Constraint**: always use `Promise.all([...])` even for single init calls. Direct `await init_X()` produces different microtask ordering than `await Promise.all([init_X()])` when sibling modules have different numbers of async deps, causing execution order to diverge from native ESM (see "Constraint" section above).

#### Concatenated module guards

The `ConcatenateWrappedModuleKind::Inner` (line 251-256) and `::Root` (line 267-276) branches return early from `visit_program` without reaching `esm_wrapper_stmt`. The `pending_init_calls` insertion must be placed BEFORE these early returns. For `Inner` modules, prepending `await Promise.all([...])` to `stmts_inside_closure` before the early return is correct — the Inner module's code is inlined into the Root module's wrapper closure, so the `await` statement appears inside the Root's async wrapper context. For `Root` modules, the same insertion applies before the Root's early return.

#### AST helper (optional)

**`crates/rolldown_ecmascript_utils/src/ast_snippet.rs`** — optionally add a `promise_all_await_stmt` helper method to encapsulate the `await Promise.all([...])` AST construction, keeping the finalizer code cleaner.

### Files not changed

| File                              | Why                                                 |
| --------------------------------- | --------------------------------------------------- |
| `compute_tla.rs`                  | `is_tla_or_contains_tla_dependency` already correct |
| `wrapping.rs`                     | `WrapKind::Esm` already correctly assigned          |
| `linking_metadata.rs`             | flag exists, no new fields needed                   |
| `ast_snippet.rs:esm_wrapper_stmt` | already supports `is_async` parameter               |
| `render_chunk_exports.rs`         | entry chunk `await` already handled                 |
| `compute_cross_chunk_links.rs`    | cross-chunk links unaffected                        |
| `code_splitting.rs`               | TLA chunk-merge prevention already in place         |

### Tests

**New fixtures** under `crates/rolldown/tests/rolldown/function/experimental/strict_execution_order/`:

- `tla_cycle/` — reproduces #9548 (circular dependency + TLA deadlock)
- `tla_concurrent_deps/` — verifies concurrent loading of independent TLA deps

**Snapshot updates**: existing TLA fixtures under `crates/rolldown/tests/rolldown/topics/tla/` may need snapshot updates if their output now uses `__esmMinAsync` instead of `__esmMin`.

## Scope and limitations

### What this fixes

- Deadlock in async init cycles (#9548)
- Sequential loading of independent async deps

### What this does not change

- `WrapKind::None` modules with TLA deps — in `strictExecutionOrder`, nearly all modules get `WrapKind::Esm`. The rare exception is on-demand wrapping optimization for pure ESM modules without side effects and no dependencies (`wrapping.rs:177-180`). These modules have no import statements to transform, so the sequential loading issue does not apply. If a `WrapKind::None` module somehow has multiple TLA deps with inline `await` init calls, those remain sequential — this is a pre-existing behavior unchanged by this design.
- `is_tla_or_contains_tla_dependency` flag granularity — could be refined to distinguish `[[HasTLA]]` from "has async dep" for more precise wrapper async-ness, but this is an optimization, not a correctness issue
- CJS wrapper + TLA — `__commonJSMin` ignores async return values from its callback; this is a separate pre-existing bug
- Concatenated wrapped modules (`ConcatenateWrappedModuleKind`) — mixed TLA/non-TLA in concatenated groups needs separate analysis
- Cross-chunk async init calls — need separate analysis for chunk-loading interaction

### Known deviations from spec

- Extra microtask ticks from `async`/`await` compared to native ESM
- No `[[CycleRoot]]` concept — each wrapper holds its own Promise independently

These deviations are inherent to any user-space TLA simulation using JavaScript Promises. Webpack shares both limitations. Execution ordering matches the spec as long as `Promise.all` is used uniformly (see "Constraint" section above).

## Related

- [runtime-helpers](./runtime-helpers.md)
- [GitHub issue #9548](https://github.com/rolldown/rolldown/issues/9548)
- [ECMAScript TLA spec](https://tc39.es/ecma262/#sec-innermoduleevaluation)
- [TC39 TLA proposal](https://github.com/tc39/proposal-top-level-await)
