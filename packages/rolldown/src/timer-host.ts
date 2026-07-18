import * as binding from './binding.cjs';
import { getRuntimeCapabilityReportCompat } from './runtime-support';
import {
  BindingMismatchError,
  isBindingMismatchError,
  markBindingMismatchError,
} from './utils/binding-mismatch-error';

type HostRegistration = readonly [high: number, low: number];
type TimerHandle = ReturnType<typeof setTimeout>;

interface CurrentThreadHostInstallation {
  taskHostRegistration?: HostRegistration;
  timerHostRegistration?: HostRegistration;
}

interface CapturedTimerHandleMethod {
  identity: unknown;
  name: string;
  run: () => void;
}

type AggregateLikeError = Error & {
  cause?: unknown;
  errors: unknown[];
};

const NativeError = Error;
const defineProperty = Object.defineProperty;
const getProperty = Reflect.get;
const construct = Reflect.construct;

const CURRENT_THREAD_HOST_INSTALLATIONS = Symbol.for(
  'rolldown.current-thread-host-installations.v4',
);
const LOCAL_CURRENT_THREAD_HOST_INSTALLATIONS = new WeakMap<
  object,
  CurrentThreadHostInstallation
>();

// See internal-docs/async-runtime/implementation.md.
function getCurrentThreadHostInstallations(): WeakMap<object, CurrentThreadHostInstallation> {
  try {
    const existing = Reflect.get(globalThis, CURRENT_THREAD_HOST_INSTALLATIONS, globalThis);
    WeakMap.prototype.get.call(existing, getCurrentThreadHostInstallations);
    if (existing !== null && (typeof existing === 'object' || typeof existing === 'function')) {
      return existing as WeakMap<object, CurrentThreadHostInstallation>;
    }
  } catch {
    // A hostile global accessor must not prevent this environment from
    // installing the native hosts it needs for CurrentThread progress.
  }

  const installations = new WeakMap<object, CurrentThreadHostInstallation>();
  try {
    if (
      Reflect.defineProperty(globalThis, CURRENT_THREAD_HOST_INSTALLATIONS, {
        configurable: true,
        value: installations,
      })
    ) {
      return installations;
    }
  } catch {
    // Duplicate native host registrations are safe. Fall back to this module
    // instance's cache when the realm-global deduplication slot is unavailable.
  }
  return LOCAL_CURRENT_THREAD_HOST_INSTALLATIONS;
}

function readHostRegistration(
  registration: unknown,
  hostLabel: string,
  contractVersion: number,
): HostRegistration {
  let high: unknown;
  let low: unknown;
  let readFailed = false;
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
    readFailed = true;
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
    throw new BindingMismatchError(
      `The loaded Rolldown binding returned an invalid CurrentThread ${hostLabel} ` +
        `registration for contract version ${contractVersion}.`,
      readFailed ? { cause: readError } : undefined,
    );
  }
  return [high, low];
}

function isHostRegistrationActive(
  registration: HostRegistration,
  isRegistrationActive: (high: number, low: number) => unknown,
  hostLabel: string,
  contractVersion: number,
): boolean {
  let active: unknown;
  let readFailed = false;
  let readError: unknown;
  try {
    active = isRegistrationActive(...registration);
  } catch (error) {
    readFailed = true;
    readError = error;
  }
  if (typeof active !== 'boolean') {
    throw new BindingMismatchError(
      `The loaded Rolldown binding returned an invalid CurrentThread ${hostLabel} ` +
        `liveness result for contract version ${contractVersion}.`,
      readFailed ? { cause: readError } : undefined,
    );
  }
  return active;
}

function readAsyncRuntimeHostExport(exportName: string): unknown {
  try {
    return Reflect.get(binding, exportName);
  } catch (error) {
    throw new BindingMismatchError(
      `The loaded Rolldown binding async-runtime host export ${exportName} could not be read. ` +
        `Reinstall Rolldown so the JavaScript package and binding versions match.`,
      { cause: error },
    );
  }
}

function invokeAsyncRuntimeHostReporter(
  exportName: string,
  reporter: (...args: never[]) => unknown,
): unknown {
  try {
    return Reflect.apply(reporter, undefined, []);
  } catch (error) {
    throw new BindingMismatchError(
      `The loaded Rolldown binding async-runtime host export ${exportName} threw while reporting. ` +
        `Reinstall Rolldown so the JavaScript package and binding versions match.`,
      { cause: error },
    );
  }
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
    // Error reporting is best effort and must not escape timer cancellation.
  }
}

function createAggregateError(
  errors: unknown[],
  message: string,
  cause: unknown,
): AggregateLikeError {
  try {
    const AggregateErrorHost = getProperty(globalThis, 'AggregateError', globalThis);
    if (typeof AggregateErrorHost === 'function') {
      return construct(AggregateErrorHost, [errors, message, { cause }]) as AggregateLikeError;
    }
  } catch {
    // Fall through to an ordinary Error that preserves the aggregate payload.
  }

  const fallback = new NativeError(message, { cause }) as AggregateLikeError;
  defineProperty(fallback, 'errors', {
    configurable: true,
    value: errors,
    writable: true,
  });
  return fallback;
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
const { capabilities: runtimeCapabilities, hasReporter: hasRuntimeCapabilityReporter } =
  getRuntimeCapabilityReportCompat();
// Shared native environments install both hosts proactively. The runtime stays
// lazy, so a synchronous pre-first-use configure call may legally switch an
// import-time MultiThread profile to CurrentThread after this module is cached.
const currentThreadHostsSupported =
  !hasRuntimeCapabilityReporter || runtimeCapabilities.asyncRuntimeBuild;

if (currentThreadHostsSupported) {
  const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 4;
  const getCurrentThreadTaskHostContractVersion = readAsyncRuntimeHostExport(
    'getCurrentThreadTaskHostContractVersion',
  );
  const isCurrentThreadHostRegistrationActive = readAsyncRuntimeHostExport(
    'isCurrentThreadHostRegistrationActive',
  );
  const registerCurrentThreadTaskHost = readAsyncRuntimeHostExport('registerCurrentThreadTaskHost');
  const registerTimerHost = readAsyncRuntimeHostExport('registerTimerHost');
  const reserveCurrentThreadHostRegistration = readAsyncRuntimeHostExport(
    'reserveCurrentThreadHostRegistration',
  );
  const unregisterCurrentThreadTaskHost = readAsyncRuntimeHostExport(
    'unregisterCurrentThreadTaskHost',
  );
  const unregisterTimerHost = readAsyncRuntimeHostExport('unregisterTimerHost');
  const hostFunctions = {
    isCurrentThreadHostRegistrationActive,
    registerCurrentThreadTaskHost,
    registerTimerHost,
    reserveCurrentThreadHostRegistration,
    unregisterCurrentThreadTaskHost,
    unregisterTimerHost,
  };
  const hostFunctionEntries = Object.entries(hostFunctions);
  const legacyHostContract =
    !hasRuntimeCapabilityReporter &&
    getCurrentThreadTaskHostContractVersion === undefined &&
    hostFunctionEntries.every(([, value]) => value === undefined);
  const completeHostContract = hostFunctionEntries.every(
    ([, value]) => typeof value === 'function',
  );
  let hostInstallation: CurrentThreadHostInstallation | undefined;
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
    throw new BindingMismatchError(
      `The loaded Rolldown binding exposes an incomplete async-runtime host contract. ` +
        `Missing or invalid exports: ${invalidExports}. Reinstall Rolldown so the JavaScript ` +
        `package and binding versions match.`,
    );
  }

  let timerHostRegistration: HostRegistration | undefined;
  try {
    if (completeHostContract) {
      const actualVersion = invokeAsyncRuntimeHostReporter(
        'getCurrentThreadTaskHostContractVersion',
        getCurrentThreadTaskHostContractVersion as () => unknown,
      );
      if (actualVersion !== CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION) {
        const actualVersionDescription =
          typeof actualVersion === 'number'
            ? String(actualVersion)
            : `a value of type ${actualVersion === null ? 'null' : typeof actualVersion}`;
        throw new BindingMismatchError(
          `The loaded Rolldown binding uses async-runtime task-host contract version ` +
            `${actualVersionDescription}, but this JavaScript package requires version ` +
            `${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}. Reinstall Rolldown so the JavaScript ` +
            `package and binding versions match.`,
        );
      }
      const hostInstallations = getCurrentThreadHostInstallations();
      const hostIdentity = registerCurrentThreadTaskHost as object;
      hostInstallation = WeakMap.prototype.get.call(hostInstallations, hostIdentity);
      if (!hostInstallation) {
        hostInstallation = {};
        WeakMap.prototype.set.call(hostInstallations, hostIdentity, hostInstallation);
      }
      const storedTaskHostRegistration = hostInstallation.taskHostRegistration;
      if (
        !storedTaskHostRegistration ||
        !isHostRegistrationActive(
          storedTaskHostRegistration,
          isCurrentThreadHostRegistrationActive as (high: number, low: number) => unknown,
          'task-host',
          CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
        )
      ) {
        hostInstallation.taskHostRegistration = undefined;
        taskHostRegistration = readHostRegistration(
          (reserveCurrentThreadHostRegistration as () => unknown)(),
          'task-host',
          CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
        );
        (
          registerCurrentThreadTaskHost as (
            registrationHigh: number,
            registrationLow: number,
          ) => unknown
        )(...taskHostRegistration);
        if (
          !isHostRegistrationActive(
            taskHostRegistration,
            isCurrentThreadHostRegistrationActive as (high: number, low: number) => unknown,
            'task-host',
            CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
          )
        ) {
          throw new BindingMismatchError(
            `The loaded Rolldown binding returned an inactive CurrentThread task-host ` +
              `registration for contract version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}.`,
          );
        }
        hostInstallation.taskHostRegistration = taskHostRegistration;
      }
    }

    if (completeHostContract && hostInstallation && !import.meta.browserBuild) {
      timerHostInstallation: {
        const storedTimerHostRegistration = hostInstallation.timerHostRegistration;
        if (
          storedTimerHostRegistration &&
          isHostRegistrationActive(
            storedTimerHostRegistration,
            isCurrentThreadHostRegistrationActive as (high: number, low: number) => unknown,
            'timer-host',
            CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
          )
        ) {
          break timerHostInstallation;
        }
        hostInstallation.timerHostRegistration = undefined;
        timerHostRegistration = readHostRegistration(
          (reserveCurrentThreadHostRegistration as () => unknown)(),
          'timer-host',
          CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
        );
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

        (
          registerTimerHost as (
            registrationHigh: number,
            registrationLow: number,
            schedule: (id: number, ms: number) => Promise<void>,
            cancel: (id: number) => void,
          ) => unknown
        )(
          ...timerHostRegistration,
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
            let timer: TimerEntry | undefined;
            try {
              timer = active.get(id);
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
                const cancellationError = createAggregateError(
                  errors,
                  unreferenced
                    ? `Rolldown CurrentThread timer ${id} could not be cancelled; ` +
                        `the timeout was unreferenced and may still fire.`
                    : `Rolldown CurrentThread timer ${id} could not be cancelled or unreferenced.`,
                  clearError,
                );
                if (unreferenced) {
                  reportTimerCancellationError(cancellationError);
                  // The callback may still run, but the active identity check makes
                  // it a no-op and unref prevents it from retaining the Node process.
                  timer.resolve();
                } else {
                  // Settle the abandoned schedule Promise for local resource release,
                  // then throw through napi-rs's catching cancellation TSFN so Rust
                  // can apply the host's bounded strike policy.
                  timer.reject(cancellationError);
                  throw cancellationError;
                }
              }
            } catch (error) {
              try {
                active.delete(id);
              } catch {}
              reportTimerCancellationError(error);
              if (timer) {
                try {
                  timer.reject(error);
                } catch (settlementError) {
                  reportTimerCancellationError(settlementError);
                }
              }
              throw error;
            }
          },
        );
        if (
          !isHostRegistrationActive(
            timerHostRegistration,
            isCurrentThreadHostRegistrationActive as (high: number, low: number) => unknown,
            'timer-host',
            CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION,
          )
        ) {
          throw new BindingMismatchError(
            `The loaded Rolldown binding returned an inactive CurrentThread timer-host ` +
              `registration for contract version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION}.`,
          );
        }
        hostInstallation.timerHostRegistration = timerHostRegistration;
      }
    }
  } catch (error) {
    const cleanupErrors: unknown[] = [];
    if (timerHostRegistration) {
      try {
        (unregisterTimerHost as (high: number, low: number) => void)(...timerHostRegistration);
        if (hostInstallation?.timerHostRegistration === timerHostRegistration) {
          hostInstallation.timerHostRegistration = undefined;
        }
      } catch (cleanupError) {
        cleanupErrors.push(cleanupError);
      }
    }
    if (taskHostRegistration) {
      try {
        (unregisterCurrentThreadTaskHost as (high: number, low: number) => void)(
          ...taskHostRegistration,
        );
        if (hostInstallation?.taskHostRegistration === taskHostRegistration) {
          hostInstallation.taskHostRegistration = undefined;
        }
      } catch (cleanupError) {
        cleanupErrors.push(cleanupError);
      }
    }
    if (cleanupErrors.length > 0) {
      const aggregate = createAggregateError(
        [error, ...cleanupErrors],
        'Rolldown host setup failed and registration rollback did not complete',
        error,
      );
      throw isBindingMismatchError(error) ? markBindingMismatchError(aggregate) : aggregate;
    }
    throw error;
  }
}
