import * as binding from './binding.cjs';

// Host integration for the `--features async-runtime` binding. CurrentThread
// runnable wakes enter through a fresh host turn instead of polling inline from
// an arbitrary Rust Waker call; timers delegate to setTimeout. Both are no-ops
// on the default tokio build.
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
const { driveCurrentThreadRuntimeTasks, registerCurrentThreadTaskHost, registerTimerHost } =
  binding as Record<PropertyKey, unknown>;
const hostFunctions = {
  driveCurrentThreadRuntimeTasks,
  registerCurrentThreadTaskHost,
  registerTimerHost,
};
const hostFunctionEntries = Object.entries(hostFunctions);
const legacyHostContract = hostFunctionEntries.every(([, value]) => value === undefined);
const completeHostContract = hostFunctionEntries.every(([, value]) => typeof value === 'function');
type CurrentThreadTaskDispatch = (dispatchHigh: number, dispatchLow: number) => void;

if (!legacyHostContract && !completeHostContract) {
  const invalidExports = hostFunctionEntries
    .filter(([, value]) => typeof value !== 'function')
    .map(([name]) => name)
    .join(', ');
  throw new TypeError(
    `The loaded Rolldown binding exposes an incomplete async-runtime host contract. ` +
      `Missing or invalid exports: ${invalidExports}. Reinstall Rolldown so the JavaScript ` +
      `package and binding versions match.`,
  );
}

if (completeHostContract) {
  (registerCurrentThreadTaskHost as (dispatch: CurrentThreadTaskDispatch) => void)(
    driveCurrentThreadRuntimeTasks as CurrentThreadTaskDispatch,
  );
}

if (
  completeHostContract &&
  !import.meta.browserBuild &&
  globalThis.setTimeout &&
  globalThis.clearTimeout
) {
  const MAX_HOST_TIMEOUT_MS = 2_147_483_647;

  type TimerEntry = {
    handle: ReturnType<typeof setTimeout> | undefined;
    remainingMs: number;
    reject: (error: unknown) => void;
    resolve: () => void;
  };

  const active = new Map<number, TimerEntry>();

  const armTimer = (id: number, timer: TimerEntry): void => {
    const delay = Math.min(timer.remainingMs, MAX_HOST_TIMEOUT_MS);
    timer.handle = globalThis.setTimeout(() => {
      if (active.get(id) !== timer) return;
      timer.remainingMs -= delay;
      if (timer.remainingMs > 0) {
        try {
          armTimer(id, timer);
        } catch (error) {
          active.delete(id);
          timer.reject(error);
        }
        return;
      }
      active.delete(id);
      timer.resolve();
    }, delay);
  };

  (
    registerTimerHost as (
      schedule: (id: number, ms: number) => Promise<void>,
      cancel: (id: number) => void,
    ) => void
  )(
    (id, ms) =>
      new Promise<void>((resolve, reject) => {
        const timer: TimerEntry = {
          handle: undefined,
          remainingMs: Math.max(ms, 0),
          reject,
          resolve,
        };
        active.set(id, timer);
        try {
          armTimer(id, timer);
        } catch (error) {
          active.delete(id);
          reject(error);
        }
      }),
    (id) => {
      const timer = active.get(id);
      if (!timer) return;
      active.delete(id);
      try {
        if (timer.handle !== undefined) {
          globalThis.clearTimeout(timer.handle);
        }
      } catch {
        // This callback crosses N-API through a non-catching TSFN. Host timer
        // cleanup failures must not escape as fatal JavaScript exceptions.
      } finally {
        // The Rust relay awaits the schedule Promise. Settle it even when a
        // host clearTimeout implementation throws so cancellation retires the
        // Rust side of the bridge.
        timer.resolve();
      }
    },
  );
}
