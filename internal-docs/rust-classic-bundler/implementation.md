# ClassicBundler

## Summary

`ClassicBundler` is the Rollup API compatibility wrapper for one-time builds. It lives in the NAPI binding layer and implements the two-step `build()` + `write()`/`generate()` pattern that Rollup exposes. Each call creates a completely fresh `BundleFactory` and `Bundle` with no shared state — no caching, no incremental rebuilds.

## The Rollup API Compatibility Problem

Rollup's JS API is:

```javascript
const bundle = await rollup(inputOptions); // build step
await bundle.write(outputOptions); // output step
bundle.close(); // cleanup
```

This is a **two-step pattern**: the build is separate from the output. Rolldown's internal `Bundle` combines both into a single operation (`write()` or `generate()` consumes the bundle). `ClassicBundler` bridges this gap by providing the Rollup-compatible surface while delegating to Rolldown's internals.

## Struct

```rust
// crates/rolldown_binding/src/classic_bundler.rs
pub struct ClassicBundler {
    session_id: Arc<str>,
    debug_tracer: Option<rolldown_devtools::DebugTracer>,
    session: rolldown_devtools::Session,
    closed: bool,
    close_future: Option<CloseFuture>,
    lifecycle: Arc<ClassicBundlerLifecycle>,
    last_bundle_handle: Option<BundleHandle>,
}
```

Each binding build entry:

1. Checks the `closed` flag — rejects if already closed
2. When devtools is first enabled, replaces the generated session ID with
   `devtools.sessionId` when provided, then creates the tracer and session span
   with that selected identity
3. Creates a **fresh `BundleFactory`** with the provided options and plugins
4. Creates a `Bundle` with `FullBuild` mode and **no cache** (`None`)
5. Creates the N-API promise that owns the bundle operation
6. Stores the `BundleHandle` for later cleanup only after promise creation
   succeeds

The last step is intentionally transactional with the JavaScript operation
owner. A synchronous bundle-construction or N-API promise-creation failure
leaves the previous handle installed, so `closeBundle` continues to target the
parallel-plugin worker pool retained by the TypeScript `RolldownBuild`.

Every native operation reserves a `ClassicBundlerOperationGuard` before
constructing its fresh `Bundle`; construction or N-API promise-creation failure
drops that reservation. A successful promise owns the guard until it settles.
The lifecycle counter and its drain waiters share one poison-recovering mutex,
so publishing the first close and admitting an operation cannot pass each
other unnoticed. After close marks the bundler closed, no later operation can
be admitted. The guard also retains a clone of the optional `DebugTracer`
lease. If JavaScript garbage-collects the raw `BindingBundler` while a promise
is active, the writer owner therefore stays registered until that operation can
no longer emit events.

Scan and `buildEnd` failures publish their diagnostic context and reserve the
same terminal-close phase before their operation guard leaves the active set.
When no unrelated operation is active, the failure-triggered `closeBundle`
retains its existing ordering and finishes before the binding promise settles.
When it would have to wait for another operation, the failed promise settles
without that wait and the close continues as generation-tracked detached work.
The tracked phase waits for those operations, runs `closeBundle`, and publishes
its outcome before releasing admission. New output entry is rejected until that
phase finishes, so the active count cannot briefly reach zero and admit another
operation while `closeBundle` is about to run. An explicit `close()` racing the
failure waits for the tracked phase and observes the same error argument.
Render/output failures do not reserve this phase, matching Rollup: they run
`renderError` and leave `closeBundle` to the later explicit bundle close.

Each failure-triggered close publishes its completed handle identity and
`closeBundle` outcome into the lifecycle state before releasing admission.
Final close waits for all of those outcomes, merges their failures in completion
order, and closes the latest handle to clear its resources. If the failure path
already closed that handle, final close suppresses the memoized hook failure
instead of appending it twice. This retains failures from older outputs after a
newer successful output replaces `last_bundle_handle`, preserves repeated
JavaScript exception identities as distinct failures, and prevents the latest
hook from being invoked twice when explicit close races a failed output.

There is no persistent state between builds. No `ScanStageCache`, no shared resolver, no reused factory.

## Key Differences from Bundler

| Aspect             | `Bundler`                    | `ClassicBundler`                   |
| ------------------ | ---------------------------- | ---------------------------------- |
| Location           | `crates/rolldown/`           | `crates/rolldown_binding/`         |
| BundleFactory      | Created once, reused         | Fresh each `create_bundle()` call  |
| ScanStageCache     | Persisted across builds      | None                               |
| SharedResolver     | Shared, cache survives       | Fresh each build                   |
| Incremental builds | Supported                    | Not supported                      |
| Use case           | Watch mode, dev mode, HMR    | Rollup-compatible `rollup()` API   |
| Close semantics    | Being refactored (see below) | User-facing `closed` flag, correct |

## Close Mechanism

The `closed` flag on `ClassicBundler` is **user-observable** — it's what `RolldownBuild.closed` checks in the JS API. This is correct and stays:

```rust
pub fn close(&mut self) -> impl Future<Output = anyhow::Result<()>> + Send + 'static {
    self.closed = true;
    // calls plugin_driver.close_bundle(None) on the last bundle handle
}

pub fn closed(&self) -> bool {
    self.closed
}
```

This is fundamentally different from `Bundler.closed`:

- **`ClassicBundler.closed`** — User-facing API contract. "This build result is done, don't call write/generate again." Correct.
- **`Bundler.closed`** — Internal hack. Exists to gate `closeBundle` calls, but `closeBundle` is a per-build concern that should live on `Bundle`. Being removed — see [rust-bundler.md](../rust-bundler/implementation.md).

Public `BindingBundler.close()` keeps the conventional N-API contract and
rejects when terminal cleanup fails. Rolldown's TypeScript facade uses the
runtime-only, declaration-hidden `BindingBundler.closeTerminal()` method
instead; it returns terminal `closeBundle` and devtools failures as structured
`BindingResult` data. A rejection from that internal path is therefore reserved
for transport/runtime failure before the terminal result was delivered.
`ClassicBundler` creates one shared close future on the first call and retains
its terminal outcome. Concurrent and later calls await or replay that same
future, so a transport failure cannot invoke `closeBundle` again or issue a
second devtools `CloseSession` request. The retained failure contains one entry
per subsystem; the binding converts each entry independently, preserving an
original JavaScript exception reference while reporting devtools failures as
separate native diagnostics. Panic containment is phase-local, so the devtools
flush is still attempted after a `closeBundle` panic and neither phase can erase
an already captured failure. The final `BundleHandle` supplies the diagnostic
`cwd`, allowing nested `BatchedBuildDiagnostic` values to expand through the
normal binding renderer instead of being flattened. N-API conversion also
contains every `Error::source()` traversal: a hostile source implementation
cannot make the memoized result permanently unconvertible, and concurrent or
later callers receive the retained failure message as a native fallback.

The shared close future first waits for every operation guard that was admitted
before close publication and every failure-triggered close derived from those
operations. Only then does it merge prior outcomes, run the final
`BundleHandle` close when necessary, and perform the acknowledged devtools
flush. This ordering makes direct
`BindingBundler.close()` calls wait for active `scan`, `generate`, and `write`
promises instead of clearing plugin resources underneath them. There is no
bundler-global "terminal hook active" shortcut: a close from an unrelated
JavaScript call must not acknowledge completion merely because another
operation is currently executing a terminal plugin callback. Reentrant close
from a plugin callback is coordinated by the public TypeScript
`CloseCallbackScope`; the lower-level raw binding does not promise reentrant
close from inside `closeBundle`.

The first close also moves the `DebugTracer` fallback guard into the shared
future. This prevents N-API object finalization from enqueueing a destructive
no-ack `CloseSession` while `closeBundle` is still running. The guard is dropped
only after the authoritative writer result has been captured; its later
best-effort close is then harmless and cannot unwind if the global writer
initializer failed or was poisoned.
The public `RolldownBuild` facade uses a declaration-hidden binding waiter for
the exact failure-triggered close phase. If a new output reaches the binding
during that gate, the facade waits for admission to reopen and retries. The
queue still releases immediately after ordinary native promise creation, so
successful concurrent outputs remain parallel. A nested `generate` or `write`
from an active close-callback scope rejects the temporary admission error
immediately instead of waiting for the same failed output whose `closeBundle`
callback is on the stack. External callers still hide that native admission
detail by waiting and retrying. Browser hosts retain the build's owner identity
until its callback results settle because they cannot propagate exact async
callback context. This prevents a nested output after an async `closeBundle`
suspension from waiting on itself, but an unrelated same-build browser caller
during that interval is indistinguishable and may receive the admission error.
Raw `BindingBundler` callers continue to receive the explicit admission error
and must coordinate retries themselves.
`RolldownBuild` and `scan()` retain the latest parallel-plugin workers and
runtime lease after such a rejection, clear only the native close
single-flight promise, and retry the binding close before releasing ownership.
After an immediate retry still fails, their public operation awaits one final
retry on a later event-loop turn. Native-close cleanup stays outside abandoned
setup recovery, and all newly delivered terminal diagnostics are deduplicated
by identity and multiplicity before rejection. Persistent transport failure
retains an explicit invalidatable cleanup claim but schedules no detached work;
successful or non-owned cleanup invalidates every error claim immediately.

## Related

- [rust-bundler](../rust-bundler/implementation.md) — Long-lived bundler for watch/dev/HMR
- `crates/rolldown_binding/src/classic_bundler.rs` — Implementation
