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

const CURRENT_THREAD_HOST_INSTALLATIONS = Symbol.for(
  'rolldown.current-thread-host-installations.v4',
);
// See internal-docs/async-runtime/implementation.md.
function getCurrentThreadHostInstallations(): WeakMap<object, CurrentThreadHostInstallation> {
  try {
    const existing = Reflect.get(globalThis, CURRENT_THREAD_HOST_INSTALLATIONS, globalThis);
    WeakMap.prototype.get.call(existing, getCurrentThreadHostInstallations);
    if (existing !== null && (typeof existing === 'object' || typeof existing === 'function')) {
      return existing as WeakMap<object, CurrentThreadHostInstallation>;
    }
  } catch {
    // An empty or foreign value in the versioned slot is not a WeakMap, so fall
    // through and install a fresh registry.
  }

  const installations = new WeakMap<object, CurrentThreadHostInstallation>();
  Reflect.defineProperty(globalThis, CURRENT_THREAD_HOST_INSTALLATIONS, {
    configurable: true,
    value: installations,
  });
  return installations;
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

function createAggregateError(errors: unknown[], message: string, cause: unknown): AggregateError {
  return new AggregateError(errors, message, { cause });
}

// Host integration for the `--features async-runtime` binding: CurrentThread
// runnable wakes enter through a fresh host turn instead of polling inline from
// an arbitrary Rust Waker call, and timers delegate to setTimeout. Both are
// no-ops on a legacy Tokio-backed binding.
//
// A side-effect module because a driver must be registered before the first
// CurrentThread `sleep_until` arms, and `getRuntimeCapabilities().timers` must
// not depend on which entry -- or which thread -- loaded the binding first.
// Registration is per-env and safe from every thread, hence no `isMainThread`
// guard: on wasm each worker owns its own driver registry, while on native the
// process-global registry races every live registrant, evicts dead ones with
// their env, and re-polls existing sleeps when a new host registers.
const { capabilities: runtimeCapabilities, hasReporter: hasRuntimeCapabilityReporter } =
  getRuntimeCapabilityReportCompat();
// Install both hosts proactively: the runtime stays lazy, so a pre-first-use
// configure call may still switch an import-time MultiThread profile to
// CurrentThread after this module is cached.
const currentThreadHostsSupported =
  !hasRuntimeCapabilityReporter || runtimeCapabilities.asyncRuntimeBuild;

if (currentThreadHostsSupported) {
  const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 4;
  const getCurrentThreadTaskHostContractVersion: unknown =
    binding.getCurrentThreadTaskHostContractVersion;
  const isCurrentThreadHostRegistrationActive: unknown =
    binding.isCurrentThreadHostRegistrationActive;
  const registerCurrentThreadTaskHost: unknown = binding.registerCurrentThreadTaskHost;
  const registerTimerHost: unknown = binding.registerTimerHost;
  const reserveCurrentThreadHostRegistration: unknown =
    binding.reserveCurrentThreadHostRegistration;
  const unregisterCurrentThreadTaskHost: unknown = binding.unregisterCurrentThreadTaskHost;
  const unregisterTimerHost: unknown = binding.unregisterTimerHost;
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
      const actualVersion = (getCurrentThreadTaskHostContractVersion as () => unknown)();
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
          clearTimeoutHost: ClearTimeoutHost;
          handle: TimerHandle | undefined;
          remainingMs: number;
          reject: (error: unknown) => void;
          resolve: () => void;
          setTimeoutHost: SetTimeoutHost;
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
              const setTimeoutHost = Reflect.get(globalThis, 'setTimeout', globalThis);
              const clearTimeoutHost = Reflect.get(globalThis, 'clearTimeout', globalThis);
              if (typeof setTimeoutHost !== 'function' || typeof clearTimeoutHost !== 'function') {
                throw new TypeError(
                  'The CurrentThread timer host requires callable global setTimeout and clearTimeout functions.',
                );
              }
              const timer: TimerEntry = {
                clearTimeoutHost: clearTimeoutHost as ClearTimeoutHost,
                handle: undefined,
                remainingMs: Math.max(ms, 0),
                reject,
                resolve,
                setTimeoutHost: setTimeoutHost as SetTimeoutHost,
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
                Reflect.apply(timer.clearTimeoutHost, globalThis, [timer.handle]);
              }
            } catch {
              // Rust invokes this callback through a non-catching TSFN. Contain
              // host cancellation failures at the JavaScript boundary, exactly
              // like the generated WASI loaders' relay.
            } finally {
              timer.resolve();
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
