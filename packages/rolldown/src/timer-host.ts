import * as binding from './binding.cjs';
import { getRuntimeCapabilitiesCompat } from './runtime-support';

type HostRegistration = readonly [high: number, low: number];
type TimerHandle = ReturnType<typeof setTimeout>;

interface CapturedTimerHandleMethod {
  identity: unknown;
  name: string;
  run: () => void;
}

function readHostRegistration(
  registration: unknown,
  hostLabel: string,
  contractVersion: number,
): HostRegistration {
  let high: unknown;
  let low: unknown;
  let readError: unknown;
  try {
    if (
      registration === null ||
      (typeof registration !== 'object' && typeof registration !== 'function')
    ) {
      throw new TypeError('registration is not an object');
    }
    high = Reflect.get(registration, 'high', registration);
    low = Reflect.get(registration, 'low', registration);
  } catch (error) {
    readError = error;
  }
  if (
    typeof high !== 'number' ||
    !Number.isInteger(high) ||
    high < 0 ||
    high > 0xffff_ffff ||
    typeof low !== 'number' ||
    !Number.isInteger(low) ||
    low < 0 ||
    low > 0xffff_ffff ||
    (high === 0 && low === 0)
  ) {
    throw new TypeError(
      `The loaded Rolldown binding returned an invalid CurrentThread ${hostLabel} ` +
        `registration for contract version ${contractVersion}.`,
      readError === undefined ? undefined : { cause: readError },
    );
  }
  return [high, low];
}

function captureTimerHandleMethod(
  handle: TimerHandle,
  key: PropertyKey,
  name: string,
): CapturedTimerHandleMethod | undefined {
  if (handle === null || (typeof handle !== 'object' && typeof handle !== 'function')) {
    return undefined;
  }
  try {
    const method = Reflect.get(handle, key, handle);
    if (typeof method !== 'function') return undefined;
    return {
      identity: method,
      name,
      run: () => {
        Reflect.apply(method, handle, []);
      },
    };
  } catch {
    return undefined;
  }
}

function captureTimerHandleFallbacks(handle: TimerHandle): {
  cancel: CapturedTimerHandleMethod[];
  unref: CapturedTimerHandleMethod | undefined;
} {
  const cancel: CapturedTimerHandleMethod[] = [];
  const identities = new Set<unknown>();
  for (const method of [
    captureTimerHandleMethod(handle, Symbol.dispose, 'timeout[Symbol.dispose]()'),
    captureTimerHandleMethod(handle, 'close', 'timeout.close()'),
  ]) {
    if (!method || identities.has(method.identity)) continue;
    identities.add(method.identity);
    cancel.push(method);
  }
  return {
    cancel,
    unref: captureTimerHandleMethod(handle, 'unref', 'timeout.unref()'),
  };
}

function reportTimerCancellationError(error: unknown): void {
  try {
    const consoleHost = Reflect.get(globalThis, 'console', globalThis);
    if (
      consoleHost === null ||
      (typeof consoleHost !== 'object' && typeof consoleHost !== 'function')
    ) {
      return;
    }
    const report = Reflect.get(consoleHost, 'error', consoleHost);
    if (typeof report === 'function') {
      Reflect.apply(report, consoleHost, [error]);
    }
  } catch {
    // Error reporting is best effort and must not escape the non-catching TSFN.
  }
}

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
//   same timer is raced across every LIVE registrant. A registrant dies with
//   its env (worker exit) and is evicted -- env-cleanup hook plus dead-callback
//   detection on the Rust side -- so a worker that imported the binding first
//   and then exited can never shadow the main thread's timers with a dead
//   callback. A newly registered host also re-polls every existing sleep, so
//   it joins timers already stranded behind a live but starved event loop.
//   Without this per-env registration, a worker that imports the binding
//   first (e.g. the parallel-plugin machinery) would be left driverless and
//   a CurrentThread sleep would panic there.
// - wasm artifacts: each worker instantiates its own wasm instance with its
//   own driver registry, so each thread MUST register its own driver.
const capabilityBinding = binding as Record<PropertyKey, unknown>;
const runtimeCapabilityGetter = capabilityBinding.getRuntimeCapabilities;
// Shared native environments install both hosts proactively. The runtime stays
// lazy, so a synchronous pre-first-use configure call may legally switch an
// import-time MultiThread profile to CurrentThread after this module is cached.
const currentThreadHostsSupported =
  typeof runtimeCapabilityGetter !== 'function' || getRuntimeCapabilitiesCompat().asyncRuntimeBuild;

if (currentThreadHostsSupported) {
  const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 2;
  const {
    getCurrentThreadTaskHostContractVersion,
    registerCurrentThreadTaskHost,
    registerTimerHost,
    unregisterCurrentThreadTaskHost,
    unregisterTimerHost,
  } = capabilityBinding;
  const hostFunctions = {
    registerCurrentThreadTaskHost,
    registerTimerHost,
    unregisterCurrentThreadTaskHost,
    unregisterTimerHost,
  };
  const hostFunctionEntries = Object.entries(hostFunctions);
  const legacyHostContract =
    getCurrentThreadTaskHostContractVersion === undefined &&
    hostFunctionEntries.every(([, value]) => value === undefined);
  const completeHostContract = hostFunctionEntries.every(
    ([, value]) => typeof value === 'function',
  );
  let taskHostRegistration: HostRegistration | undefined;

  if (
    !legacyHostContract &&
    (!completeHostContract || typeof getCurrentThreadTaskHostContractVersion !== 'function')
  ) {
    const invalidExports = hostFunctionEntries
      .filter(([, value]) => typeof value !== 'function')
      .map(([name]) => name)
      .concat(
        typeof getCurrentThreadTaskHostContractVersion === 'function'
          ? []
          : ['getCurrentThreadTaskHostContractVersion'],
      )
      .join(', ');
    throw new TypeError(
      `The loaded Rolldown binding exposes an incomplete async-runtime host contract. ` +
        `Missing or invalid exports: ${invalidExports}. Reinstall Rolldown so the JavaScript ` +
        `package and binding versions match.`,
    );
  }

  if (completeHostContract) {
    const actualVersion = (getCurrentThreadTaskHostContractVersion as () => unknown)();
    if (actualVersion !== CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION) {
      throw new TypeError(
        `The loaded Rolldown binding uses async-runtime task-host contract version ` +
          `${String(actualVersion)}, but this JavaScript package requires version ` +
          `${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}. Reinstall Rolldown so the JavaScript ` +
          `package and binding versions match.`,
      );
    }
    const registration = (registerCurrentThreadTaskHost as () => unknown)();
    taskHostRegistration = readHostRegistration(
      registration,
      'task-host',
      CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
    );
  }

  let timerHostRegistration: HostRegistration | undefined;
  try {
    if (completeHostContract && !import.meta.browserBuild) {
      const MAX_HOST_TIMEOUT_MS = 2_147_483_647;

      type SetTimeoutHost = (callback: () => void, delay: number) => TimerHandle;
      type ClearTimeoutHost = (handle: TimerHandle) => void;
      type TimerEntry = {
        cancelHandleFallbacks: CapturedTimerHandleMethod[];
        clearTimeoutHost: ClearTimeoutHost;
        handle: TimerHandle | undefined;
        remainingMs: number;
        reject: (error: unknown) => void;
        resolve: () => void;
        setTimeoutHost: SetTimeoutHost;
        unrefHandle: CapturedTimerHandleMethod | undefined;
      };

      const active = new Map<number, TimerEntry>();

      const armTimer = (id: number, timer: TimerEntry): void => {
        const delay = Math.min(timer.remainingMs, MAX_HOST_TIMEOUT_MS);
        const handle = Reflect.apply(timer.setTimeoutHost, globalThis, [
          () => {
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
          },
          delay,
        ]) as TimerHandle;
        timer.handle = handle;
        const fallbacks = captureTimerHandleFallbacks(handle);
        timer.cancelHandleFallbacks = fallbacks.cancel;
        timer.unrefHandle = fallbacks.unref;
      };

      const timerRegistration = (
        registerTimerHost as (
          schedule: (id: number, ms: number) => Promise<void>,
          cancel: (id: number) => void,
        ) => unknown
      )(
        (id, ms) =>
          new Promise<void>((resolve, reject) => {
            if (!timerHostRegistration) {
              throw new TypeError('The CurrentThread timer host registration is unavailable.');
            }
            const setTimeoutHost = Reflect.get(globalThis, 'setTimeout', globalThis);
            const clearTimeoutHost = Reflect.get(globalThis, 'clearTimeout', globalThis);
            if (typeof setTimeoutHost !== 'function' || typeof clearTimeoutHost !== 'function') {
              throw new TypeError(
                'The CurrentThread timer host requires callable global setTimeout and clearTimeout functions.',
              );
            }
            const timer: TimerEntry = {
              cancelHandleFallbacks: [],
              clearTimeoutHost: clearTimeoutHost as ClearTimeoutHost,
              handle: undefined,
              remainingMs: Math.max(ms, 0),
              reject,
              resolve,
              setTimeoutHost: setTimeoutHost as SetTimeoutHost,
              unrefHandle: undefined,
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
          if (timer.handle === undefined) {
            timer.resolve();
            return;
          }

          try {
            Reflect.apply(timer.clearTimeoutHost, globalThis, [timer.handle]);
            timer.resolve();
            return;
          } catch (clearError) {
            const errors = [clearError];
            for (const fallback of timer.cancelHandleFallbacks) {
              try {
                fallback.run();
                reportTimerCancellationError(
                  new Error(
                    `Rolldown CurrentThread timer ${id} clearTimeout failed; ` +
                      `the timeout was cancelled with ${fallback.name}.`,
                    { cause: clearError },
                  ),
                );
                timer.resolve();
                return;
              } catch (fallbackError) {
                errors.push(fallbackError);
              }
            }

            let unreferenced = false;
            if (timer.unrefHandle) {
              try {
                timer.unrefHandle.run();
                unreferenced = true;
              } catch (unrefError) {
                errors.push(unrefError);
              }
            }
            const cancellationError = new AggregateError(
              errors,
              unreferenced
                ? `Rolldown CurrentThread timer ${id} could not be cancelled; ` +
                    `the timeout was unreferenced and may still fire.`
                : `Rolldown CurrentThread timer ${id} could not be cancelled or unreferenced.`,
              { cause: clearError },
            );
            reportTimerCancellationError(cancellationError);
            if (unreferenced) {
              // The callback may still run, but the active identity check makes
              // it a no-op and unref prevents it from retaining the Node process.
              timer.resolve();
            } else {
              // Rejecting is the only remaining error channel. Rust treats this
              // as a live-host relay failure and applies its bounded strike policy.
              timer.reject(cancellationError);
            }
          }
        },
      );
      timerHostRegistration = readHostRegistration(
        timerRegistration,
        'timer-host',
        CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
      );
    }
  } catch (error) {
    const cleanupErrors: unknown[] = [];
    if (timerHostRegistration) {
      try {
        (unregisterTimerHost as (high: number, low: number) => void)(...timerHostRegistration);
      } catch (cleanupError) {
        cleanupErrors.push(cleanupError);
      }
    }
    if (taskHostRegistration) {
      try {
        (unregisterCurrentThreadTaskHost as (high: number, low: number) => void)(
          ...taskHostRegistration,
        );
      } catch (cleanupError) {
        cleanupErrors.push(cleanupError);
      }
    }
    if (cleanupErrors.length > 0) {
      throw new AggregateError(
        [error, ...cleanupErrors],
        'Rolldown host setup failed and registration rollback did not complete',
        { cause: error },
      );
    }
    throw error;
  }
}
