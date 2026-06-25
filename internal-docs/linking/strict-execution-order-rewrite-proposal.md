# Proposal: rewrite the strictExecutionOrder / wrapped-ESM init model

> Status: **draft for discussion**; migration steps 1–2 started. Anchored on #9961;
> #9691/PR#9709 is the same strict-order family. #9806 shares only the _symptom_ (lazyBarrel +
> retained body) and is **not** fixed here — it needs the separate loader-time invariant in §5.
>
> **Implementation progress (steps 1–2):**
>
> - Step 1: regression fixture `tests/rolldown/function/experimental/strict_execution_order/binding_init_unused_named_import`
>   (per-binding activation guard) added; green.
> - Step 2: shadow-mode obligation pass `crates/rolldown/src/stages/link_stage/init_obligations.rs`,
>   called from `link()` after `patch_module_dependencies`. **Env-gated** behind
>   `ROLLDOWN_DUMP_INIT_OBLIGATIONS` so it's a no-op on normal builds (output unchanged by
>   construction; strict suite 24/24 green). Validated: on #9961 `main` → `[setupWorker.js,
http.js]` (BindingInit on canonical owners), **no `core`** — vs current `[core]`; on the
>   per-binding fixture `main` → `[used.js]`, no `unused.js`.
> - Step 2 broad validation: ran the shadow diff across the **whole** `strict_execution_order`
>   suite (83 builds). Graph-level red-flag = a module init'd by the current model but given no
>   obligation anywhere under the new model **and** side-effectful (Ordering would have to catch
>   it). Result: **`RISKY=0` on every build.** All dropped inits are pure barrels/proxies
>   (`barrel.js`, `proxy.js`, `ns-proxy.js`, `lib-foo.js`, `outer.js`, …) replaced by canonical
>   owners — the intended improvement; no side-effectful init is ever dropped.
> - **Caveat investigated → resolved (no regression).** Added two guard fixtures: a
>   `sideEffects:false` barrel importing a real side effect
>   (`sideeffect_free_barrel_reexports_sideeffectful`) and a non-waived twin
>   (`sideeffectful_barrel_reexports_sideeffectful`). Findings: (a) a `sideEffects:false`
>   barrel's transitive side effect is **already dropped by current strict** (Rollup's "module
>   _and its imports_" rule) — current == rewrite == dropped, consistent with default; (b) a
>   non-waived barrel **keeps** the side effect under both models (shadow `dropped=0`) — the
>   barrel earns an `Ordering` obligation and survives. So pruning never drops a _non-waived_
>   side effect; the §7 item is about placement, not correctness loss.
> - **Step-3 touch-points confirmed (code-level):** fixing #9961 is a _coordinated_ change —
>   (1) `module_finalizers/mod.rs:392-410` (`WrapKind::Esm` arm) emits `init_<importee>()`
>   **directly**, no canonical-following → must follow canonical owners when the importee is a
>   pruned/pure barrel (the `WrapKind::None` arm already does this via
>   `wrapped_esm_init_stmt_for_import_record`); (2) the **inclusion** decision (stop
>   force-including the barrel via its wrapper ref) must live in `include_statements`, **not**
>   `reference_needed_symbols`: the latter runs `par_iter` after `std::mem::take(&mut
self.symbols)`, so each closure has only its own module's symbol db — cross-module
>   `canonical_ref_for` is unavailable there. `include_statements` has the restored symbol db
>   and already canonical-resolves in `include_symbol`.
> - Step 3 (started, gated): coordinated edits behind env flag `ROLLDOWN_INIT_OBLIGATIONS`
>   (off by default → byte-identical; strict suite 27/27 green flag-off, zero tracked-snapshot
>   drift). (a) `reference_needed_symbols.rs` `WrapKind::Esm` arm skips the importee's `init`
>   ref when the importee is side-effect-free, not re-export-all, and not namespace-imported —
>   so a `sideEffects:false` barrel is no longer force-included; the canonical owner is init'd
>   via the binding-use path. (b) `module_finalizers/mod.rs` `WrapKind::Esm` arm canonical-
>   follows (`wrapped_esm_init_stmt_for_import_record`) when the importee is pruned.
>   **Result: #9961 is FIXED flag-on** — `core` pruned, no `checkGlobals`, no `init_core`;
>   output == strict-off (modulo the entry wrapper).
> - **Step 3 gap found (flag-on), diagnosed precisely:** `exports_chain_indirect_ns` panicked
>   (`code_splitting.rs:910`, `proxy.js` `is_included` but empty bits). Chain:
>   `main: import {star} from ns-proxy-2` → … → `ns-proxy: import * as star from './proxy'` →
>   `proxy: export {foo,bar}` → lib-foo (`value='foo'+v`). Mechanism: `ns-proxy`'s
>   `import * as star` keeps `init_proxy` (force-including `proxy`), but the skip prunes
>   `proxy`'s downstream `init_proxy_foo` ref **and** `main`'s `star.foo` resolves by _member
>   resolution_ straight to `value` (lib-foo), bypassing `proxy`'s `foo` export — so every one
>   of `proxy`'s statements is pruned → included-but-empty. Two hard parts: (1) the init
>   obligation must follow **member resolution** (`star.foo` → `value` → lib-foo), not the
>   import binding (`star` → proxy's namespace); (2) `ns-proxy-2` is **locally
>   indistinguishable** from #9961's `core` (both `sideEffects:false` barrels re-exporting a
>   `Literal`) — the namespace-ness is transitive (2 hops), so a per-edge decision in
>   `reference_needed_symbols` (no cross-module symbols) cannot distinguish them.
> - **Step 3 gap RESOLVED (conservative namespace-taint guard), flag-on suite now green:** added
>   a precompute in `reference_needed_symbols` that builds the **weakly-connected import
>   components** (undirected over import-record edges) and seeds a taint on every component
>   containing a star-import edge (`import * as ns`, detected via `named_imports … is_star()`).
>   The skip is disabled for any module in a tainted component. #9961's chain is named-import-
>   only (no star edge) ⇒ untainted ⇒ still prunes (`checkGlobals` gone); namespace chains
>   revert to the current eager wrapping ⇒ byte-identical to flag-off ⇒ no stranding, no panic.
>   **Result flag-on:** whole `strict_execution_order` suite **27/27 green, zero panics**;
>   `exports_chain` prunes cleanly (`init_proxy/init_proxy_foo/init_dep` collapse to a direct
>   `init_lib_foo()`); `exports_chain_indirect_ns` snapshot **does not drift** (taint reverts
>   it). Flag-off unchanged: full integration suite **1764 passed / 0 failed**, zero tracked-
>   snapshot drift. The taint is a _safety floor_, not the final model — worst case it reverts a
>   namespace subgraph to today's behavior; it never prunes a materialized namespace.
> - **Step 3 consolidated as a first-class, tested option.** Promoted the `ROLLDOWN_INIT_OBLIGATIONS`
>   env flag to a real `experimental.wrappedModuleTreeshaking` option (`ExperimentalOptions.wrapped_module_treeshaking`
>   - `is_wrapped_module_treeshaking_enabled()`; wired into `reference_needed_symbols` and the finalizer;
>     `ConfigVariant.wrapped_module_treeshaking` for test variants; napi `BindingExperimentalOptions` field +
>     `TryFrom` map; `_config.schema.json` auto-regenerated). Off by default. **Committed
>     config-variant regression coverage** locks in the model's behavior: `issues/9961` snapshots
>     the bug (base, `checkGlobals` dangling) _and_ the fix (`[wrapped_module_treeshaking: true]` → `core`
>     pruned, no `checkGlobals`) side by side; `exports_chain` shows the barrel prune
>     (`init_proxy/init_proxy_foo/init_dep` → direct `init_lib_foo()`); `exports_chain_indirect_ns`
>     shows the namespace-taint revert (no panic); `binding_init_unused_named_import` shows the
>     per-binding barrel prune (no `unused`); the two `*_barrel_reexports_sideeffectful` fixtures
>     prove side-effect soundness (non-waived kept, waived dropped — in _both_ variants). Full
>     integration suite **1764 / 0** flag-off; strict suite **27/27**.
> - **JS-API exposure wired (source-complete).** `wrappedModuleTreeshaking?: boolean` added to the TS
>   `ExperimentalOptions` (`input-options.ts`), the valibot schema (`validator.ts`), the
>   `bindingify` map (`bindingify-input-options.ts`), and the generated `binding.d.cts`
>   (`BindingExperimentalOptions`, matching napi's field order). `tsc -p tsconfig.json --noEmit`
>   passes; no JS test enumerates the options set (non-breaking). Runtime activation in JS needs a
>   `build-binding` (native rebuild) — a normal deploy/CI step; the Rust integration tests already
>   prove the behavior, so it's not gating.
> - **Flag-on sweep (default-on across the _whole_ suite, then reverted).** Temporarily flipped
>   `is_wrapped_module_treeshaking_enabled()` to default-`true` and ran the full integration suite to map
>   gaps #2/#3 (cross-chunk / broad behavior). **Result: 0 panics, 0 execution failures across
>   1764 tests** — the surgical fix never crashes, even under code-splitting/cross-chunk/CJS-
>   interop. **Blast radius is tiny: only 8 snapshots change** (snapshot diffs auto-update so they
>   don't fail; failures = panics/exec only). 4 are the dedicated strict fixtures; of the other 4,
>   **3 are still `strictExecutionOrder`** (plus code-splitting / cjs) and **1 is genuinely
>   non-strict** (corrected — an earlier draft wrongly called all 4 non-strict):
>   - ✅ `advanced_chunks/include_dependencies_recursively` — `strictExecutionOrder` **+
>     code-splitting**; correct empty-wrapper prune (init_bar no-op dropped, `foo` intact).
>   - ✅ `issues/9028` — `strictExecutionOrder`; correct empty-barrel prune.
>   - ⚠️ `topics/exports/common_esm_named_named` — `strictExecutionOrder` **+ cjs + code-splitting**;
>     correct but **changes the chunk graph**: pruning the empty `shared.js` barrel collapses a shared
>     chunk + the runtime chunk into `main`. Not a bug, but chunk-structure churn that flipping the
>     default would need to own.
>   - ✅ `esbuild/use_strict_directive_bundle_issue1837` — **the one genuinely non-strict case**
>     (`iife` + `inject`, no strict order — the empirical witness that the gate reaches non-strict
>     wrapping) and **was the one real defect; now FIXED.**
>     `shims.js` is `import process from 'process'; export { process }` (re-exports an **external**
>     binding); the skip pruned the side-effect-free barrel and **dropped the external `process`
>     import still referenced downstream** (`cjs.js`/`entry.js`) — no `ReferenceError` (it's a
>     global/external), but `process` silently fell back to the ambient global → wrong value.
>     **Same class as #9961, but for an external/global target** — the canonical-follow can't save
>     it (owner is external, not `WrapKind::Esm`). **Fix:** added a final guard to `skip_wrapper_ref`
>     in `reference_needed_symbols` — never skip a barrel that directly imports from any external
>     module (short-circuits on the env flag, so flag-off is unaffected). Re-sweep confirms: blast
>     radius drops 8→7 (the esbuild snapshot is unchanged flag-on), still 0 panics/exec failures,
>     and the committed `wrappedModuleTreeshaking` variants are unchanged (27/27 strict, #9961 green, zero
>     drift). The remaining transitive case (barrel → barrel → external, ≥2 hops) is a known
>     conservative gap, like the namespace taint; no fixture exercises it.
> - **Execution coverage confirmed (migration step 1 substantially met).** `expect_executed` and
>   `write_to_disk` both default `true`, and the harness executes **per variant** (the
>   `for named_options in multiple_options` loop runs node on each variant's output), so every
>   `[wrapped_module_treeshaking: true]` config variant is **node-executed**, not just snapshot-compared.
>   The model's correctness is therefore runtime-asserted: `exports_chain` runs
>   `nodeAssert.equal(value, "foo")`, `exports_chain_indirect_ns` runs `nodeAssert.equal(star.foo,
"foo")`, `sideeffect_free_barrel_reexports_external` runs `nodeAssert.equal(typeof sep,
"string")`, `binding_init_unused_named_import` runs (no `ReferenceError`). #9961's expect*executed:
>   false (its buggy base can't run) is the \_only* non-executed case, and its fix is covered
>   structurally (snapshot: `checkGlobals` absent) + by the executed analogues above. This corrects
>   the earlier "step 1 unmet" worry — snapshots are _backed by_ execution here.
> - **Milestone: the incremental link-stage phase is essentially complete.** Flag-on is
>   crash-free, silent-wrong-free, execution-tested, and regression-locked across the whole suite.
>   The remaining strict-order steps are all _large or policy_: (a) the member-resolution namespace
>   prune (the body-symbol emission engine + §7 placement — migration steps 4-5); (b) retire
>   reconciliation (step 5, not yet safe — the surgical fix didn't build the edge pipeline that
>   would subsume `transitive_esm_init_targets`); (c) flip the default (a shipped-behavior policy
>   call — the evidence now supports it: 0 crashes, 7 benign deltas, execution-backed). The
>   separate lazyBarrel #9806 loader invariant (§5) is also ready as an independent track.
> - **Reframed as a wrapping-module impl gate (renamed `initObligations` → `wrappedModuleTreeshaking`).**
>   The defect is _not_ strictExecutionOrder-specific: a module gets `WrapKind::Esm` from **any**
>   wrap trigger — `strictExecutionOrder`, `require(esm)`, code-splitting / CJS interop — and the
>   over-retention bug rides along each. The gate already lives in the `WrapKind::Esm` arm (not
>   gated on strict order), and the sweep witnessed it: of the 8 changed fixtures, the genuinely
>   non-strict one was `esbuild/use_strict_directive_bundle_issue1837` (`iife` + `inject`). So the
>   option is the **legacy-vs-new wrapped-ESM side-effect tree-shaking** switch for _all_ wrap
>   paths, and is named accordingly (`experimental.wrappedModuleTreeshaking`, off = legacy).
>   Implication: because the bug reaches **default builds** (a `require(esm)` of a
>   `sideEffects:false` barrel + lazyBarrel deferral is the same `ReferenceError` class as #9961, no
>   strict order needed), the new impl has broader value than strictExecutionOrder alone — a
>   stronger eventual case for making it default. The internal model/diagnostic keeps the
>   "obligation" name (`init_obligations.rs`, `ROLLDOWN_DUMP_INIT_OBLIGATIONS`); only the public
>   option is renamed.
> - **Broad scope now locked with a dedicated non-strict fixture (DONE).**
>   `function/experimental/wrapped_module_treeshaking/require_esm_barrel` has **no
>   strictExecutionOrder**: `require('./consumer.mjs')` wraps `consumer.mjs` (`WrapKind::Esm` via
>   `require(esm)`), and the recursive wrap reaches the `sideEffects:false` barrel it named-imports
>   `{ used }` from. Base (legacy) keeps `init_barrel = __esmMin(() => { init_used(); })`; the
>   `[wrapped_module_treeshaking: true]` variant **prunes the barrel** and calls `init_used()`
>   directly (canonical-follow). Executes (`require` interop) and asserts `consumer.result ===
'used-value'` — so a regression is caught at runtime, not just in the snapshot. (Note: _pure_
>   esm code-splitting doesn't wrap — modules hoist into chunks; non-strict wrapping needs
>   `require`/cjs interop, which this fixture exercises. The strict `advanced_chunks` /
>   `topics/exports` fixtures cover the strict×code-splitting combination.)
> - TODO: tighten the conservative taint into the real member-resolution-aware
>   `BindingInit`/`NamespaceInit` (needs **body-symbol-driven init emission** — confirmed absent:
>   every emission site is import-record/re-export-driven — plus the §7 first-dominating-site
>   placement; this is migration steps 4-5, deliberately deferred); always-on shadow +
>   diff-as-warning surfacing.

## 1. The defect we keep re-paying for

`output.strictExecutionOrder` emulates esbuild's model: instead of hoisting modules into
one flat scope (Rollup's model, rolldown's default), every ESM module body is wrapped in
an `init_<module>()` closure (`__esm`/`__esmMin`), and `init_<importee>()` calls are
emitted so modules initialize in ESM execution order.

The model couples **two subsystems that must agree statement-by-statement**:

1. **inclusion / tree-shaking** — `tree_shaking/include_statements.rs`. Honors
   `sideEffects:false` as a _module-pruning_ hint.
2. **wrapping + ordering** — `wrapping.rs`, `reference_needed_symbols.rs`, the finalizers.
   Decides which `init_*()` to emit, and **ignores `sideEffects:false`**.

When they disagree you get one of two failures:

- **emit a call to something tree-shaking dropped** → `ReferenceError`
  (#9691 `export *`, #9806 `const X = Imp.Y`, #9961 bare `call()`);
- **drop a call ordering needed** → wrong execution order.

Each bug so far is one _statement shape_ the model failed to reconcile, fixed by one more
special case. That is the fragility.

### 1.1 #9961, traced precisely

```
core/package.json   { "sideEffects": false }
core/index.js       import { checkGlobals } from './checkGlobals.js';
                    checkGlobals();                       // top-level side effect
                    export { setupWorker } from './setupWorker.js';
                    export { http } from './http.js';
main.js             import { setupWorker, http } from './core/index.js';
                    const worker = setupWorker(http()); console.log(worker);
```

- **strict OFF** (verified): `core` is pruned _as a whole module_ — `setupWorker`/`http`
  resolve to canonicals in their own files, and `core` is `sideEffects:false` so nothing
  reaches it. `checkGlobals` appears nowhere. **Correct.**
- **strict ON** (verified): `init_core()` is chained into `main`; `core` is force-included;
  `checkGlobals()` is retained (correctly — once a module is in, its side-effect statements
  run, by design); but lazyBarrel deferred `./checkGlobals.js`, so the definition is gone →
  `ReferenceError`.

The force-include chain, exact:

1. `wrap_modules` (strict) sets `core`, `setupWorker.js`, `http.js` → `WrapKind::Esm`.
2. `reference_needed_symbols.rs:177` (`Import` / `WrapKind::Esm` / not-reexport) attaches
   `init_core` to `main`'s `import { setupWorker, http } from './core'` statement, with
   `side_effect = core.has_side_effects() = false`.
3. In `include_statements`, using `setupWorker` includes its **declaring statement**
   (`include_statements.rs:1048-1064`) — and `setupWorker` is a _named import_ in `main`,
   so its declaration site is that import statement. It is force-included.
4. The import statement references `init_core` → `include_symbol(init_core)` →
   `include_module(core)` (`include_statements.rs:875-896`).
5. `core` is now included; `checkGlobals()` (`side_effect=true`) is kept; lazyBarrel already
   nulled `./checkGlobals.js` (`resolved_module=None`), so the finalizer drops the import
   (`module_finalizers/mod.rs:308`) and the call dangles.

**Root cause:** the init reference is attributed to the _syntactic barrel the import names_
(`core`), not the _canonical owner of the binding actually read_ (`setupWorker.js`). A
`sideEffects:false` re-export barrel has **no binding-init obligation** (its bindings'
values live elsewhere) and **no ordering obligation** (side effects waived), yet the model
force-includes it.

> Note: `sideEffects:false` is a _module-reachability_ hint, **not** statement-level purity.
> Once a module is included for any reason, its top-level side-effect statements run — by
> design, matching esbuild/webpack. So the fix must live at **inclusion**, never at
> statement retention. (Do not "fix" by stripping `checkGlobals()` from an included module.)

### 1.2 The reconciliation patches (the cost of the current model)

- `reference_needed_symbols.rs` force-marks wrapped-import statements `side_effect=true`;
  #9709 had to carve out a 4-condition lazy exception for `export *`.
- `generate_stage/compute_wrapped_esm_init_metadata.rs` is an **entire pre-finalization
  pass** (`init_is_noop`, `transitive_esm_init_targets`) whose only job is to _re-create_
  init calls tree-shaking dropped and `@__PURE__`-drop ones it kept needlessly.
- `module_finalizers/mod.rs`: `add_wrapped_esm_init_module_for_symbol` has 4 guards
  (`WrapKind::Esm && is_included && wrapper_is_reachable_in_chunk && !concatenated-inner`);
  `wrapper_is_reachable_in_chunk` exists solely so a synthesized call never names a wrapper
  absent from the chunk.
- `transform_or_remove_import_export_stmt:308` drops imports with `resolved_module=None`.

Churn evidence: `wrapping.rs` carries #4670 (reverted by #4686), #4687, #4692, #5240,
#5305, #5498, #5729; `strict_execution_order/issue_*` is a graveyard of point fixes
(4636, 4684, 4782, 4920, 5303, 5922, 8777, 8910) plus 9691/9961. (#9806 is the lazyBarrel-only
cousin — same symptom, no `strictExecutionOrder`; see §5.)

## 2. The reframe: module liveness vs. initialization obligations

Two independent facts, deliberately **not** collapsed into one (the original draft collapsed
"included" and "init target" — too restrictive; see below):

- **`ModuleLive(X)`** — X's body/wrapper must exist in the output. Established by the
  _existing_ inclusion machinery, which has many entry points beyond "a binding is read":
  dynamic entries (`process_and_retain_dynamic_entry`), namespace objects
  (`module_namespace_included_reason`), `preserveModules`
  (`preserve_reexported_interfaces`), HMR (dev-mode namespace inclusion,
  `include_statements.rs:792`), `require(esm)` (`__toCommonJS` arm), entry exports
  (`referenced_symbols_by_entry_point_chunk`), and `NoTreeshake` modules. The rewrite does
  **not** redefine liveness — a binding use establishes `ModuleLive` through ordinary symbol
  inclusion exactly as today.
- **`InitBefore(site, X, reason, await)`** — X must be _initialized_ before `site` executes.
  This is the thing `init_X()` emission is derived from, and it is what the rewrite
  restructures.
  - **`site` is a static position, not a read expression.** It is an import-record /
    module-initialization point — concretely, a slot in the importer's `__esm` init prelude.
    Init calls are emitted there in **ESM dependency order**, never inline at each binding
    read. (esbuild's model: the wrapper body opens with ordered `init_*()` calls.)
  - **The obligations form an _ordered_ relation, not just an edge set.** Order is part of
    the data: for `A` importing side-effectful re-export barrel `B` forwarding canonical `X`,
    the required init order is `X` → `B` → `A`. The producer must emit obligations carrying
    enough order (the importer's import-record index, keyed against `exec_order`) for the
    finalizer to place calls deterministically.

  `reason` is one of:
  - **BindingInit** — a specific imported binding whose **canonical** lives inside X's wrapped
    closure is read. Modeled per-binding as a **candidate `(trigger_symbol, target=canonical
owner)`**, _activated_ only when `trigger_symbol ∈ used_symbol_refs`
    (`include_statements.rs:821,866`). Statement-level survival is too coarse: for
    `import { used, unused } from './barrel'` the import statement survives for `used`, but
    `unused`'s candidate must stay inactive so its canonical owner isn't over-initialized.
    Target is the canonical owner, resolved through re-export barrels — never the syntactic
    import source. (This is the #9961 fix.)
  - **Ordering** — X must run its top-level side effects in order. Exists **only** when X
    has a non-waived top-level side effect (not `sideEffects:false`, not otherwise provably
    pure).
  - **NamespaceInit** (a.k.a. ExportMaterialization) — X's _namespace value_ is observed and
    the runtime must build it: `__reExport(...)` copies, dynamic CJS `export *` whose members
    can't be mapped to static canonical owners, `import * as ns`. **This obligation is never
    waived by side-effect metadata** — a `sideEffects:false` module still has an observable
    namespace if someone enumerates/spreads it. Treat it as a non-waivable sibling of
    BindingInit (BindingInit's target is a _symbol_; NamespaceInit's target is the _namespace
    object_).

  `await` is set when X is TLA-tainted.

Emission rule:

- For each active obligation targeting X, emit `init_X()` (or `await init_X()`) at the
  **first dominating site along each execution path** in ESM dependency order — the earliest
  init-prelude position on that path that precedes every reader/side-effect of X.
  `ModuleLive(X)` is a precondition for the call (the wrapper must exist), but liveness alone
  never _creates_ a call.

The dominating site is **per entry/importer execution path, not global.** Two independent
entries that both import X may share no common dominating site; the correct result is to emit
`init_X()` once in _each_ importer's prelude and let `__esm`'s `fn = 0` idempotence collapse
the redundant run at runtime. Idempotence is what makes _cross-path redundancy_ safe.

Within a single path, placement is a **correctness requirement, not an optimization**:
idempotence guarantees X's body runs at most once, but it does **not** guarantee the _first_
call on that path is positioned before X's first observer — which is exactly what breaks under
cycles and TLA if the obligation order is wrong. See §7.

So #9961's `core` can be `ModuleLive` for some unrelated reason without ever being an
`InitBefore` target — and conversely, the fix is that nothing makes `core` either.

> **Inclusion is necessary but not sufficient for emission.** A module being `is_included`
> (whole-bundle) does **not** prove its wrapper is reachable from the chunk being finalized;
> cross-chunk wrapper references are registered separately
> (`compute_cross_chunk_links.rs:323`, `add_depended_symbol_with_wrapped_esm_init`). So the
> `InitBefore` edges must flow through the _whole_ lowering pipeline (statement inclusion →
> `patch_module_dependencies` → chunk reachability → cross-chunk wrapper import/export
> registration → post-`on_demand_wrapping` lowering), and the chunk-reachability guard
> (`wrapper_is_reachable_in_chunk`) stays — fed by the edge set, not deleted because the
> target is `is_included`. What the model _can_ eventually retire is the _reactive
> re-creation_ (`transitive_esm_init_targets`), once edges are computed up front.

### 2.1 #9961 under the new model

`main` reads `setupWorker`/`http`; their canonicals are in `setupWorker.js`/`http.js` ⇒
BindingInit obligations targeting **those** files (→ `init_setupWorker`, `init_http`, both
empty closures ⇒ `init_is_noop` ⇒ dropped). `core`: no BindingInit (no canonical of `core`'s
own is read), no NamespaceInit (its namespace is never observed), no Ordering
(`sideEffects:false`). ⇒ no `InitBefore(*, core)` ⇒ no `init_core()`. Nothing else makes
`core` live ⇒ pruned ⇒ `checkGlobals()` gone with the module. Output equals strict-off. No
statement-stripping, no dangling call.

`#9691` (`export *`) resolves the same way: the star source earns a BindingInit per used
star-exported canonical, a NamespaceInit if its namespace is observed, and Ordering only if
it has a real side effect — replacing the #9709 carve-out. **`#9806` is _not_ fixed by this
rewrite** (it does not use `strictExecutionOrder`); it shares only the lazyBarrel/retained-
body symptom and needs the separate loader-time invariant in §5.

## 3. Where it maps onto the pipeline

Current `LinkStage::link` order (`link_stage/mod.rs:228`):

```
sort_modules → compute_tla → determine_module_exports_kind → determine_safely_merge_cjs_ns
→ wrap_modules → generate_lazy_export → determine_side_effects → bind_imports_and_exports
→ create_exports_for_ecma_modules → reference_needed_symbols → cross_module_optimization
→ include_statements → patch_module_dependencies
```

Key fact: `determine_side_effects` runs **after** `wrap_modules`, so wrapping can't consult
final side effects today (relevant to the §6 wrap-skip optimization). But `reference_needed_symbols` and
`include_statements` run **after** it, so the obligation computation has side effects
available.

Functions in scope:

| Concern                          | Today                                                                            | Change                                                                                                                                                                                                                     |
| -------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| attach init ref to import stmt   | `reference_needed_symbols.rs:175-192`                                            | emit **BindingInit to canonical owner** + **NamespaceInit** when the namespace is observed + **Ordering only if importee has non-waived side effect**; stop attaching `init_<syntactic-barrel>` for pure re-export barrels |
| force-include via declaring stmt | `include_statements.rs:1048-1064` + `875-896`                                    | unchanged for liveness (canonical-owner inclusion already works); the bug disappears once the stray `init_<barrel>` ref is gone                                                                                            |
| chunk-level wrapper linking      | `compute_cross_chunk_links.rs:323` (`add_depended_symbol_with_wrapped_esm_init`) | obligation edges must drive cross-chunk wrapper import/export registration — they are not finalizer-local                                                                                                                  |
| re-create dropped init calls     | `compute_wrapped_esm_init_metadata.rs` (`transitive_esm_init_targets`)           | retire **only after** edges provably subsume it (migration step 5), not before                                                                                                                                             |
| finalizer init emission + guards | `module_finalizers/mod.rs:161-298`                                               | drive from edge sets; **keep** `wrapper_is_reachable_in_chunk` (whole-bundle `is_included` ≠ per-chunk reachability)                                                                                                       |
| wrap decision                    | `wrapping.rs:163-188`                                                            | **not** downgraded by side-effect metadata; only by a proven-no-op-wrapper / `ExecutionOrderSensitive`+leaf check (separate optimization, §6)                                                                              |

The finalizer **already** has canonical-owner-following machinery
(`collect_wrapped_esm_init_modules_for_import_record` →
`add_wrapped_esm_init_module_for_symbol`, `mod.rs:161-235`) — the rewrite generalizes it
from the `WrapKind::None` fallback to the primary path and feeds it from precomputed edges.

## 4. Migration plan (incremental, suite stays green)

The current output is _known-wrong_ for #9961, so "the new model reproduces today's
emission" is **not** a valid gate — the model is supposed to differ. And reconciliation
(`transitive_esm_init_targets`, the chunk-reachability guard) cannot be removed until the
canonical-owner obligations actually replace the syntactic-barrel references. Hence:

1. **Execution tests first.** Add _node-execution_ regression tests (not just snapshots) for
   every case in §8 — they encode the real invariant (no `ReferenceError`, correct order),
   which snapshots alone don't.
2. **Compute candidate obligations in shadow mode.** Add a pass (after
   `reference_needed_symbols`, consuming `determine_side_effects`) that builds the
   `InitBefore` edge set (BindingInit / NamespaceInit / Ordering). Do **not** assert exact
   equivalence with today; instead diff against current emission and _record the expected
   deltas_ (#9961 etc.). No output change yet.
3. **Integrate behind a flag — atomically.** Wire the edges into statement inclusion,
   `patch_module_dependencies`, chunk reachability, and cross-chunk wrapper registration.
   Note "keep fallbacks _and_ output unchanged" is **not achievable** once edges affect
   inclusion/chunk dependencies — partial integration desyncs the two systems. So either keep
   this step _fully shadow-only_ (edges computed, no consumer reads them) **or** put the whole
   switch (inclusion + chunk linking + emission) behind a single flag and flip all consumers
   atomically. Do not half-integrate.

   Confirmed touch-points (code-level):
   - **Inclusion** — `include_statements.rs`. When a used named import resolves (via
     `canonical_ref_for`, available here) to a canonical owner in a _different_ module than the
     syntactic importee, include the **owner's** wrapper instead of force-including a
     `sideEffects:false` pass-through barrel. This is where pruning is actually decided.
     **Not** `reference_needed_symbols` — it runs `par_iter` after `std::mem::take(&mut
self.symbols)`, so cross-module canonical resolution is unavailable in that pass.
   - **Emission** — `module_finalizers/mod.rs:392-410` (`WrapKind::Esm` arm) emits
     `init_<importee>()` directly; make it follow canonical owners when the importee is
     pruned, reusing `wrapped_esm_init_stmt_for_import_record` /
     `collect_wrapped_esm_init_modules_for_import_record` (the `WrapKind::None` arm's helper).
   - **Cross-chunk** — `compute_cross_chunk_links.rs:323` must register the _canonical owners'_
     wrappers, not the barrel's.

4. **Flip the flag + land the intended deltas.** Enable the flagged path from step 3: the
   obligation producer (`reference_needed_symbols.rs:175`) attributes BindingInit to the
   canonical owner (not the syntactic barrel), emits NamespaceInit where the namespace is
   observed, and Ordering only for non-waived side-effect importees, and all consumers read the
   edge set. **This is where #9961 is fixed** and the snapshot deltas predicted in step 2
   materialize. Make `9961` default to the new path and verify it against the strict-off
   baseline.
5. **Retire reconciliation once proven redundant.** Remove `transitive_esm_init_targets` and
   any now-dead guard _only after_ the edge invariants demonstrably cover them (the
   `wrapper_is_reachable_in_chunk` guard stays unless its job is fully subsumed by edge-driven
   cross-chunk registration). Each removal keeps the suite green.
6. **(Separate, optional) No-op wrapper / wrap-skip optimization.** See §6 — gated on a
   proven-unnecessary init body, **not** on side-effect metadata.

Correctness lands at step 4; steps 5–6 are cleanup/perf.

## 5. Edge cases to keep honest

- **`export *` / `__reExport`** — `__reExport(...)` is **NamespaceInit, not Ordering**: it
  constructs an observable namespace value and may be required even when both modules are
  `sideEffects:false`, so it must **not** be waivable by side-effect metadata. Static members
  resolve to BindingInit on their canonical owners; dynamic CJS `export *` that can't be
  mapped statically falls back to NamespaceInit on the namespace object. Ordering is added
  only if the star source has a real side effect. (This is the correction to the original
  draft, which wrongly called the `__reExport` arm an Ordering edge.)
- **TLA** — every obligation kind to a TLA-tainted target carries an `await` flag (today's
  `is_tla_or_contains_tla_dependency` branch in `wrapped_esm_init_call_expr`).
- **CJS — explicitly out of scope.** `require_foo` / CJS wrappers keep their current behavior
  unchanged; the rewrite restructures only the ESM-wrap (`init_*`) path. Do **not** assume CJS
  "can't waive side effects": today a CJS importee's import is `side_effect =
importee.side_effects.has_side_effects()` (`reference_needed_symbols.rs:149`), so package
  `sideEffects:false` already affects CJS imports. Leave that path as-is rather than folding it
  into the obligation model.
- **Circular deps** — idempotence (`__esm`'s `fn = 0`) makes _redundant re-entry_ harmless,
  but it does **not** by itself make a cycle correct: the first `init_*()` on each path must
  still be **placed** before the first observer (§2), and a back edge in the cycle means some
  binding is unavoidably read mid-initialization. The obligation order must encode the cycle's
  entry edge so placement matches the runtime live-binding semantics; idempotence only covers
  the repeat, not the placement.
- **Concatenation** (`ConcatenateWrappedModuleKind`) — an edge to an inner module maps to the
  group's shared `init_*`; preserve the current inner/none/outer handling.
- **lazyBarrel — separate, loader-time invariant (cannot be a link-stage edge).** lazyBarrel
  defers records _during module loading_ and writes `resolved_module=None`
  (`module_loader.rs:475`), **before** the scan completes and long before canonical owners or
  link-stage `InitBefore` edges exist. So a link-stage edge cannot "un-defer" an unscanned
  dependency, and #9961's link-stage fix does **not** transitively fix the lazyBarrel cases.
  The loader needs its **own** invariant: never defer a record whose binding is read by a
  statement that will be kept once the barrel is live — decided from _requested exports_ plus
  "does evaluating this local export require the module body?" This is the change that
  actually fixes **#9806** (which doesn't use `strictExecutionOrder` at all) and removes the
  lazyBarrel half of #9961; it needs its own regression tests (§8) and is tracked separately
  from the strict-order rewrite.
  - **IMPLEMENTED.** `BarrelState::initialize_barrel_tracking` (`lazy_barrel.rs`) now takes
    `stmt_infos` and, after `take_needed_records`, force-loads any still-deferred record whose
    binding is read by a **side-effectful** barrel-own statement (`symbol_ref()` of each
    `referenced_symbols` entry → `named_imports` → record). The side-effect filter excludes import
    declarations + source-less export clauses, so plain re-export deferral is untouched. Regression:
    `tests/rolldown/issues/9806` (was `ReferenceError`, now bundles the CJS sub-module + executes
    clean). Full suite **1766/0**, blast radius = **1** (only #9961, whose `core` has the same shape:
    its base now loads `checkGlobals.js` → no crash, and the fixture flips to `expectExecuted:true`;
    the `[wrapped_module_treeshaking]` variant still prunes `core` entirely). lazyBarrel optimization preserved
    everywhere else.
- **preserveModules** — `preserve_reexported_interfaces` (`include_statements.rs:681`) already
  re-includes re-export facades on used chains; reconcile so a facade kept for interface
  reasons establishes liveness only, never a spurious Ordering obligation.

## 6. Separate optimization: wrap-skip (NOT correctness)

The original draft proposed downgrading `sideEffects:false` modules to `WrapKind::None`.
That is **unsafe**: a module can be externally side-effect-free yet have observable
_evaluation timing_ — through its exported value under a cycle, through TLA, or through
re-entrancy ordering. `sideEffects:false` is about side effects, not about whether the
lazy-init wrapper is needed.

So wrap-skip must be gated on the init **body being provably unnecessary**, using constraints
comparable to the existing `EcmaViewMeta::ExecutionOrderSensitive` + leaf checks
(`wrapping.rs:38-44, 177-186`) or a proven no-op wrapper — **never** on package side-effect
metadata. Note the `init_is_noop` pass already drops empty wrappers _post hoc_ via
`@__PURE__` + dce-only, so this is purely an up-front size/clarity optimization and is not
required for #9961 correctness. Pursue it independently of the rewrite.

## 7. Open questions

- First-dominating-site placement is now a **model requirement** (§2 emission rule), not an
  open question. What remains open is the _implementation_: today's dedup
  (`generated_init_esm_importee_ids`) is per-finalized-module; the edge model needs a concrete
  algorithm to compute, **per execution path**, the earliest init-prelude position dominating
  X's observers in `exec_order` — and to prove it correct for sync cycles and TLA.
- `exec_order` is computed before inclusion (`sort_modules` is first); confirm pruning a
  module post-inclusion doesn't desync any `exec_order`-keyed consumer
  (`module-execution-order/implementation.md` §Downstream Consumers).
- Cost of the edge pass vs. the (eventually) retired `transitive_esm_init_targets` pass
  (expected net neutral-to-positive).

## 8. Verification

Baseline: `strict_execution_order/**`, `issues/{9691,9806,9961}`, and the esbuild
snapshot-diff. Add **node-execution** coverage (snapshots alone don't catch `ReferenceError`
or wrong order) for, at minimum:

- namespace **property access** vs. namespace **enumeration / spread** (BindingInit vs.
  NamespaceInit);
- dynamic CJS `export *` (no static canonical owner → NamespaceInit fallback);
- TLA propagated through multiple barrels (`await` flag transitivity);
- synchronous **and** asynchronous (TLA) cycles;
- cross-chunk canonical owners and **manual chunks** (exercises cross-chunk wrapper
  registration / `wrapper_is_reachable_in_chunk`);
- `preserveModules`;
- `require(esm)` and dynamic `import()` (code-splitting on **and** off);
- earliest-call placement when several sites target the same wrapper;
- lazyBarrel × {enabled, disabled} × code-splitting × {enabled, disabled} (the loader-time
  invariant, §5).

After step 4, `9961` should contain no `checkGlobals` and flip to `expectExecuted:true`.
The lazyBarrel/#9806 fixtures gate on the _separate_ loader-time change, not the strict-order
rewrite.
