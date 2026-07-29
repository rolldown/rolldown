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
callback scope, and a throwing getter preserves its original error. The final
native Promise may observe a non-function accessor again while adopting the
identity, but no wrapper is introduced, so private fields and `WeakMap` keys
remain valid. Data-property thenables use the same assimilation and cycle
detection. Build and dev callback settlement share this resolver.

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
- `builtin-plugin/utils.ts` maintains exhaustive callback-key inventories for
  callback-bearing native built-ins and wraps each configured callback.
  Accessor-backed callback properties are read once inside the boundary and
  replaced with data properties before N-API converts the options. Wrappers are
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
  conversion.

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
- `scripts/misc/check-browser-runtime-contract.mjs` verifies callback-free
  operation without a provider, fail-closed callback entry, a real
  `AsyncLocalStorage` provider, reentrancy and concurrency, configuration
  locking, and absence of `node:async_hooks` in browser artifacts.
- Focused tests cover same-identity and fresh-proxy prototype chains in option
  discovery and built-in option access.
- `scripts/misc/check-wasi-binding-packed-consumer.mjs` launches the packed
  browser package in Chromium, verifies the public preflight error for a direct
  callback data property, proves provider state survives an `await`, and makes
  the reentrant build attempt after that continuation.

## Related

- [design.md](./design.md) - why host-backed context and fail-closed behavior are required
- [async runtime implementation](../async-runtime/implementation.md) - scheduler and host runtime integration
