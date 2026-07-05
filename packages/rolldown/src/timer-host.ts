import { registerTimerHost } from './binding.cjs';

// Timer host for the `--features async-runtime` binding: its CurrentThread
// flavor delegates timers (e.g. the watch-mode debounce) to the host event
// loop. A no-op on the default tokio build.
//
// This lives in its own side-effect module because every public entry that
// loads the binding needs it (library entry via `setup.ts`, the CLI, and the
// direct-binding entries like `rolldown/experimental`): a driver must be
// registered before the first CurrentThread `sleep_until` arms, and the
// capability contract (`getRuntimeCapabilities().timers`) must not depend on
// which entry -- or which THREAD -- loaded the binding first.
//
// Deliberately NO `isMainThread` guard (unlike setup.ts's trace subscriber,
// whose main-thread-only reasons do not apply here -- worker event loops
// have setTimeout too). Registration is safe and required from every thread:
// - native addon: the process-global driver REGISTRY (rolldown_utils
//   `TimerDriverRegistry`) takes one registration per importing env, and the
//   newest LIVE registrant serves each timer. A registrant dies with its env
//   (worker exit) and is evicted -- env-cleanup hook plus dead-callback
//   detection on the Rust side -- so a worker that imported the binding
//   first and then exited can never shadow the main thread's timers with a
//   dead callback; in-flight sleeps re-arm on the next live registrant.
//   Without this per-env registration, a worker that imports the binding
//   first (e.g. the parallel-plugin machinery) would be left driverless and
//   a CurrentThread sleep would panic there.
// - wasm artifacts: each worker instantiates its own wasm instance with its
//   own driver registry, so each thread MUST register its own driver.
if (!import.meta.browserBuild && globalThis.setTimeout && globalThis.clearTimeout) {
  type TimerEntry = {
    handle: ReturnType<typeof setTimeout>;
    resolve: () => void;
  };

  const active = new Map<number, TimerEntry>();

  const registerTimerHostWithCancel = registerTimerHost as unknown as (
    schedule: (id: number, ms: number) => Promise<void>,
    cancel: (id: number) => void,
  ) => void;

  registerTimerHostWithCancel(
    (id, ms) =>
      new Promise<void>((resolve) => {
        const handle = globalThis.setTimeout(() => {
          active.delete(id);
          resolve();
        }, ms);
        active.set(id, { handle, resolve });
      }),
    (id) => {
      const timer = active.get(id);
      if (!timer) return;
      active.delete(id);
      globalThis.clearTimeout(timer.handle);
      // The Rust relay awaits the schedule Promise. Resolve it after clearing
      // the host timeout so cancellation retires both sides of the bridge.
      timer.resolve();
    },
  );
}
