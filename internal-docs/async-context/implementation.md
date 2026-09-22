# Build Callback Async Context - Implementation

> The rationale and invariants live in [design.md](./design.md).

## Provider Selection

`packages/rolldown/src/utils/async-context.ts` owns the platform abstraction.

- Node creates `AsyncLocalStorage` instances.
- Browser builds first use a provider installed by
  `configureAsyncContext(provider)`.
- Without a configured provider, browser builds use
  `globalThis.AsyncContext.Variable` when available.
- If neither exists, entering a protected callback throws
  `AsyncContextUnavailableError` with code
  `ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE`.

The required context is lazy. Importing the browser package and running a build
that has no user callbacks does not require a provider. The first protected
callback creates storage and locks configuration only after that creation
succeeds. Failed required and optional acquisitions leave configuration open so
the host can install a provider and retry. Configuration is blocked during
provider validation and the entire acquisition call stack, so an accessor or
provider cannot replace itself reentrantly while an outer operation is still
selecting it. `configureAsyncContext()` snapshots the validated `createStorage`
method with its original receiver and rejects if getter side effects selected a
provider before validation completed. The first acquisition records its provider
candidate for the whole synchronous call stack. If a native
`AsyncContext.Variable` constructor reenters context creation after replacing
the global constructor, the nested acquisition reuses the original candidate
instead of selecting a different provider. Optional context creation locks only
after it successfully creates storage, so callback-free browser builds do not
freeze an unavailable selection. All created contexts in the evaluated module
still use one stable source. `getAsyncContextSupport()` reports the currently
selectable or selected source by creating and discarding a probe storage. The
probe validates only the storage method shape, does not lock configuration, and
is never used by a build.

The provider contract requires `run()` to preserve its store through promises
and `await`, equivalent to Node.js `AsyncLocalStorage` or
`AsyncContext.Variable`. Rolldown cannot dynamically prove this semantic
property. A stack-only implementation is invalid even though it has the same
method shape.

For browser one-shot builds, `createBundlerOptions()` inspects the converted
binding options for plugin, logging, input, output, and built-in-plugin
callbacks. When any are present, it enters an empty `BuildCallbackRunner`
invocation before the binding call. This selects the provider or throws the
public `AsyncContextUnavailableError` directly in JavaScript, before emnapi can
replace its name or code. Callback-free builds skip this preflight, and a failed
preflight leaves provider configuration open for a retry.

## Invocation Chain

`RolldownBuild` owns a process-wide context whose store is a linked invocation:

```ts
interface BuildCallbackInvocation {
  active: boolean;
  build: RolldownBuild;
  parent: BuildCallbackInvocation | undefined;
}
```

`#runBuildCallback()` pushes a node, invokes the callback, and deactivates the
node when the callback result settles. `#isActiveBuildCallback()` walks the
ancestry chain. This catches direct and indirect cycles such as
`A callback -> B build -> B callback -> A build`, while a caller outside the
chain can still start a concurrent build.

N-API callback entry does not preserve the JavaScript caller's asynchronous
context. Each `#build()` therefore captures its initiating invocation and closes
over it in that build's `BuildCallbackRunner`. When native code later invokes a
callback without a current store, the runner uses the captured invocation as
the parent. A nested callback that already has a current store uses that more
specific chain instead.

The promise finalizer is created inside the selected async context. This keeps
the invocation visible through the callback's `await` continuations. Once the
callback settles, descendants that retained the context see an inactive node
and no longer block builds. Promise-like callback results are assimilated
through a cached `then` function. Each captured custom `then` method keeps
Promise-like deferred invocation timing and enters a fresh close-callback scope
for the exact method call and synchronous resolving-function work. A nested
value's `then` is therefore inspected when `resolve(value)` is called, before
later microtasks can mutate it, while nested method invocation remains a later
Promise job. This lets a close-capable callback request close from `then()`
without granting browser microtasks the same privilege. The custom resolver
rejects self-resolution and mutual thenable cycles before the native Promise
algorithm can spin indefinitely. It also tracks the final promise returned to
the caller, so resolving a custom thenable with that public promise rejects as a
cycle instead of leaving the callback active forever.

A direct callback result whose accessor-backed `then` reads as a non-function
is returned unchanged. Nested accessor-backed values follow Promise resolution
semantics without proxying: a non-function fulfills with the original object
identity, a callable getter result is cached and assimilated under the selected
callback scope, and a throwing getter preserves its original error. The settled
value travels through the resolver inside a `{ value }` box, so no intermediate
promise runs the Promise Resolution Procedure on it. `utils/async-flatten.ts`
boxes plugin-option values the same way (its box also carries the captured
`then` and the cycle chains) and follows the same procedure - one `then` read
per step, job-deferred invocation inside the scope, resolve-time
classification - so a plugin option and a callback result assimilate
identically; there a callable `then` also takes precedence over array
flattening. Only the promise handed back to the caller unboxes
and adopts the value, which limits an accessor-backed `then` to one
classification read plus that single adoption. The box never escapes and no
wrapper is introduced, so private fields and `WeakMap` keys remain valid.
Deactivation runs before the final adoption, so a `then` that only turns
callable during it cannot leave the callback active. Data-property thenables use
the same assimilation and cycle detection. Build and dev callback settlement
share this resolver.

A plain native promise takes a shorter path on Node builds. When the cached
`then` is the built-in `Promise.prototype.then` captured at module load and the
result passes `util.types.isPromise` (a brand check a `Proxy` fails without
reaching a trap), has the intrinsic prototype, and has no own `constructor`,
calling `then` runs no user code. The tracker then attaches it synchronously
instead of in a later job, and hands each fulfilled value to the same
resolve-time classification as above, so `then` reads, cycle checks and
deactivation-before-adoption are unchanged; only the extra promises and
microtask turns go away. Every caller invokes the tracker inside the context
its `runSynchronousCallback` enters, so the reactions see the same store.
Browser builds have no trap-free brand check and always use the general path.

The shared `utils/prototype-chain.ts` walker is used by logger/output-hook
discovery and callback-bearing built-in option access. It tracks visited
identities and allows at most 256 inspected objects. Cyclic proxies therefore
throw a deterministic `TypeError`; proxies that manufacture a fresh prototype
on every lookup fail at the same bounded depth instead of blocking the isolate.

`CloseCallbackScope` selects its optional async-context storage on the first
callback invocation rather than during module evaluation. Browser hosts can
therefore import Rolldown and then call `configureAsyncContext()` before any
callback runs. If no provider is available, the scope retains its synchronous
browser fallback and retries selection on a later invocation. Watcher
close-listener dispatch uses the same lazy selection rule, so importing the
watch API cannot lock it to the broader browser fallback before host
configuration.

`DevEngine` owns a separate process-wide context whose invocation identifies an
engine-specific owner token. The `onOutput`, `onHmrUpdates`, and
`onAdditionalAssets` adapters enter that context before invoking user code and
deactivate it when the callback's synchronous result or returned promise
settles. `DevEngine.close()` walks the active ancestry and rejects a same-engine
callback before publishing the closing state. This prevents the cycle where
native work awaits the callback while close awaits that admitted work, without
blocking an unrelated caller from closing the engine.

## Callback Boundary

`createBundlerOptions()` passes one `BuildCallbackRunner` through both binding
option adapters.

- `bindingify-plugin.ts` wraps every build and output plugin hook.
- `CloseCallbackScope.wrapCallbacks()` then wraps every function left in the
  binding options with the close-callback scope. `RolldownBuild`'s runner
  already runs each callback through that same scope, so a wrapper that calls
  the runner first - plugin hooks, functional `external`, built-in option
  callbacks, and the composed logger - is marked with
  `markScopeEnteringCallback()` and handed over unwrapped. One hook call
  therefore enters the close scope once. Watch and dev pass no runner, mark
  nothing, and keep the `wrapCallbacks()` wrapper as their only scope entry.
  Output-option wrappers in `bindingify-output-options.ts` are not marked yet
  and still enter the scope twice.
- `builtin-plugin/utils.ts` maintains exhaustive callback-key inventories for
  callback-bearing native built-ins and wraps each configured callback.
  Every callback key's value comes from exactly one `[[Get]]` - a
  `Reflect.get` with the original receiver - which is the same operation
  N-API's `napi_get_named_property` performs, so the binding sees the same
  value a JS reader performing that `[[Get]]` at that moment would see. The
  moment is not the one N-API would have picked. The callback keys are read
  here, during option conversion, before the native call reads anything; a
  config handed to N-API untouched had all of its fields read later, inside
  the native call, in the binding struct's field order. That order was never
  a contract, so a trap whose answer for one key depends on an earlier read
  of another key now sees its conversion-time answer pinned.
  The descriptor the bounded walk
  found decides only whether the read calls user code: an accessor that has a
  getter is read inside the boundary, while every other shape - a data
  property, a getter-less accessor, or a key no descriptor answered for - is
  the same single read taken outside it, so a callback-free config stays inert
  and needs no provider. A getter-less accessor is read like the rest rather
  than skipped, because there is nothing to call and a `get` trap may still be
  what answers for the key. A `Proxy` `get` trap that answers such a plain
  read is user code too, and it runs outside the boundary on purpose: the
  boundary covers callback execution, not option reads, and guarding the read
  would demand a provider for every callback-free build. A trap that starts a
  build of the same bundle from inside that read is therefore not rejected;
  this is the same as a config handed to N-API untouched, where the trap ran
  inside the native call with no guard at all. A callback a `Proxy` serves from its `get` trap is
  therefore wrapped rather than handed to N-API raw, even when the target owns
  a rival descriptor for that key. The snapshot overlay is now the only
  wrapped shape, whatever found the keys: rebuilding the options as a plain
  object from the original's own descriptors dropped every field that only a
  `get` trap can answer, so a config whose callback is an own property lost its
  trap-served siblings. The overlay is unconditional: the pass reads every
  callback key exactly once and always pins what it read, so N-API never
  performs a second read on the original, and a callback-free config still
  needs no provider because building the overlay runs no user code. The overlay
  answers every callback key - function, non-function, or `undefined` - from
  that first read, so N-API's `napi_get_named_property` cannot reach a trap a
  second time and collect a raw callback, and it delegates every other property
  to the original object with the original receiver, so required fields the
  same trap serves still reach the binding. The overlay owns the snapshot on a
  private target, which keeps the `Proxy` invariants satisfiable over a frozen
  config.
  Wrappers are
  installed even when no build runner is present because N-API may invoke them
  as detached functions; each wrapper applies the callback with its original
  options object as the receiver.
- `bindingify-input-options.ts` wraps functional `external`,
  `treeshake.moduleSideEffects`, while `create-bundler-option.ts` wraps the
  composed logger when it contains a plugin or user `onLog`/`onwarn` callback.
- `bindingify-output-options.ts` wraps addon functions, file-name functions,
  `sanitizeFileName`, `globals`, `paths`, sourcemap callbacks, asset naming,
  and code-splitting `name`/`test` callbacks. The deprecated `manualChunks`
  callback reaches the same boundary through its generated code-splitting
  group.
- The `outputOptions` plugin hook runs through the runner before binding option
  conversion. Whether a plugin supplies `outputOptions` - or `onLog` - is
  decided by the value the capture read, judged with the same truthiness test
  the hook runner applies before calling it, so presence and execution never
  disagree: a hook only a `get` trap can answer for is guarded, and a falsy
  answer no runner would ever call leaves a callback-free build unguarded. Only
  an accessor that has a getter is deferred, because calling a getter is user
  code the walk can see in advance; that read happens inside the boundary,
  once per snapshot, and counts as present on its own. A `Proxy` `get` trap
  that answers the eager read is user code too, and it runs outside the
  boundary for the reason given for built-in options above: the boundary
  covers hook execution, not option reads. The list handed to the hook runner
  and to the logger is one view per plugin: an internal-only facade whose
  `Proxy` target is a fresh empty object, so no invariant can force a live
  answer, and whose traps answer the hook key from the captured value and
  delegate every other key to the plugin with the plugin as the receiver - so
  `plugin.name`, which both consumers read unsnapshotted, still reaches a getter
  backed by a private field. A hook the plugin gains or replaces after the
  capture is not observed, which is what keeps presence and execution in
  agreement: a late hook answered live would run outside the guard the presence
  flags installed. The private-target trick is safe only because no consumer of
  these views is user code; a view handed to user code keeps the original as its
  target instead (`createWatchOptionSnapshotView`, see
  [../watch-mode/implementation.md](../watch-mode/implementation.md)).

Internal callbacks such as deferred scan-data collection and cache invalidation
are not wrapped because they do not invoke user code. This distinction keeps
callback-free browser builds operational without weakening the user callback
contract.

## Public API

The experimental entry exports:

- `configureAsyncContext(provider)`
- `getAsyncContextSupport()`
- `AsyncContextUnavailableError`
- `AsyncContextProvider`, `AsyncContextStorage`, and `AsyncContextSupport`

The provider must return storage with `getStore()` and `run(store, callback)`.
`run()` must preserve the store across asynchronous continuations. Support
reporting validates method shape only; it does not claim to prove propagation.
Node rejects configuration because its provider is fixed.

## Verification

- Node build API tests cover direct `generate`, `write`, and `close`
  reentrancy, output option callbacks, unrelated concurrent calls, indirect
  cycles, and detached descendants.
- Dev tests cover `close()` after an asynchronous continuation in `onOutput`
  and `onAdditionalAssets`, including the lazy `compileEntry()` path.
- `packages/browser-tests/runtime-contract.mjs` verifies callback-free
  operation without a provider, fail-closed callback entry, a real
  `AsyncLocalStorage` provider, reentrancy and concurrency, configuration
  locking, and absence of `node:async_hooks` in browser artifacts.
- Focused tests cover same-identity and fresh-proxy prototype chains in option
  discovery and built-in option access.
- `scripts/wasi/check-wasi-binding-packed-consumer.mjs` launches the packed
  browser package in Chromium, verifies the public preflight error for a direct
  callback data property, proves provider state survives an `await`, and makes
  the reentrant build attempt after that continuation.

## Related

- [design.md](./design.md) - why host-backed context and fail-closed behavior are required
- [async runtime implementation](../async-runtime/implementation.md) - scheduler and host runtime integration
