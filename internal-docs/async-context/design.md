# Build Callback Async Context - Design & Principles

## Summary

Rolldown must reject a build, write, or close call that originates from one of
the same bundle's active JavaScript callbacks. Dev engines likewise reject
`close()` from one of their active output callbacks. Allowing either call to
enter native cleanup can create a self-wait cycle. At the same time, unrelated
callers must remain free to start concurrent work or close normally. The
implementation therefore tracks callback ancestry with host-backed asynchronous
context rather than global activity counters. See
[implementation.md](./implementation.md) for the control flow and callback
inventory.

## Design Principles

1. **Reject provenance cycles, not concurrency.** A callback may call another
   bundle, and an outside caller may start a build while a callback is
   suspended. Only an active ancestry chain that reaches the same bundle is
   rejected.
2. **Cover every native-invoked user callback.** Plugin hooks, input and output
   option functions, output-options hooks, and user log handlers share one
   boundary. Protecting only plugin hooks leaves equivalent deadlocks in options
   such as `external`, `banner`, and `manualChunks`.
3. **Track the callback promise lifetime.** An invocation remains active through
   asynchronous continuations and becomes inactive when its returned thenable
   settles. Detached descendants may build after that settlement.
   The same rule lets detached dev callback descendants close after the
   callback itself has completed.
4. **Require a real host primitive.** Node uses `AsyncLocalStorage`. Browser
   hosts may configure an equivalent provider or expose
   `AsyncContext.Variable`. Promise patching, task counters, timing heuristics,
   and promise tagging cannot reliably preserve provenance through native
   `await`. Method-shape validation cannot prove this semantic guarantee, so
   provider documentation must state it explicitly.
5. **Fail before user code when provenance cannot be represented.** A browser
   without an async-context provider may still run callback-free builds. The
   first protected callback throws
   `ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE` before the callback is invoked.
6. **Freeze configuration on first use.** Provider selection stays lazy so a
   host can configure the browser package after import, but it cannot change
   after protected callback execution begins.

## Related

- [implementation.md](./implementation.md) - provider selection, callback wrapping, and tests
- [async runtime implementation](../async-runtime/implementation.md) - native scheduler and browser runtime
