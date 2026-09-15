import { installCurrentThreadHosts } from '@napi-rs/async-runtime';

import * as binding from './binding.cjs';

// Host integration for the shared runtime: CurrentThread runnable wakes enter
// through a fresh host turn instead of polling inline from an arbitrary Rust
// Waker call, and timers delegate to setTimeout.
//
// A side-effect module because a driver must be registered before the first
// CurrentThread `sleep_until` arms, and `getRuntimeCapabilities().timers` must
// not depend on which entry -- or which thread -- loaded the binding first.
// Registration is per-env and safe from every thread, hence no `isMainThread`
// guard: on wasm each worker owns its own driver registry, while on native the
// process-global registry races every live registrant, evicts dead ones with
// their env, and re-polls existing sleeps when a new host registers.
//
// Install proactively: the runtime stays lazy, so a pre-first-use configure
// call may still switch an import-time MultiThread profile to CurrentThread
// after this module is cached.
//
// Only the native loader needs this. The generated WASI loaders install the
// same hosts themselves from `napi.wasm.asyncRuntime` (@napi-rs/cli >= 3.10.0),
// and the deferred workerd loader installs per-instance hosts instead.
// See internal-docs/async-runtime/implementation.md.
installCurrentThreadHosts(binding, {
  // Browser timer support remains a separate capability decision: the wasm
  // build reports `timers: false` and never arms a host sleep.
  installTimerHost: !import.meta.browserBuild,
});
