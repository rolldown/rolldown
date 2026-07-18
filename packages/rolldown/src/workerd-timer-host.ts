const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 4;

type TimerHostRegistration = (
  registrationHigh: number,
  registrationLow: number,
  schedule: (id: number, ms: number) => Promise<void>,
  cancel: (id: number) => void,
) => void;

interface WorkerdTimerBinding {
  getCurrentThreadTaskHostContractVersion: () => unknown;
  isCurrentThreadHostRegistrationActive: (high: number, low: number) => unknown;
  registerTimerHost: TimerHostRegistration;
  reserveCurrentThreadHostRegistration: () => unknown;
  unregisterTimerHost: (high: number, low: number) => void;
}

/**
 * Register the CurrentThread timer bridge for one managed workerd instance.
 *
 * The optional second schedule argument remains accepted by the local relay
 * implementation so its timer behavior can be exercised independently.
 */
export function registerWorkerdTimerHost(binding: WorkerdTimerBinding): () => void {
  const setTimeoutHost = globalThis.setTimeout?.bind(globalThis);
  const clearTimeoutHost = globalThis.clearTimeout?.bind(globalThis);
  if (!setTimeoutHost || !clearTimeoutHost) return () => {};

  const getCurrentThreadTaskHostContractVersion = Reflect.get(
    binding,
    'getCurrentThreadTaskHostContractVersion',
  );
  const isCurrentThreadHostRegistrationActive = Reflect.get(
    binding,
    'isCurrentThreadHostRegistrationActive',
  );
  const reserveCurrentThreadHostRegistration = Reflect.get(
    binding,
    'reserveCurrentThreadHostRegistration',
  );
  const unregisterTimerHost = Reflect.get(binding, 'unregisterTimerHost');
  if (
    typeof getCurrentThreadTaskHostContractVersion !== 'function' ||
    typeof isCurrentThreadHostRegistrationActive !== 'function' ||
    typeof reserveCurrentThreadHostRegistration !== 'function' ||
    typeof unregisterTimerHost !== 'function'
  ) {
    throw new TypeError('The managed workerd binding does not support exact timer-host disposal');
  }
  const actualVersion = Reflect.apply(getCurrentThreadTaskHostContractVersion, binding, []);
  if (actualVersion !== CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION) {
    throw new TypeError(
      `The managed workerd binding uses CurrentThread task-host contract version ` +
        `${String(actualVersion)}, but version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION} is required`,
    );
  }

  const MAX_HOST_TIMEOUT_MS = 2_147_483_647;

  type TimerEntry = {
    handle: ReturnType<typeof setTimeout> | undefined;
    remainingMs: number;
    reject: (error: unknown) => void;
    resolve: () => void;
  };

  const active = new Map<number, TimerEntry>();
  const legacyActive = new Set<TimerEntry>();
  let registration: [number, number] | undefined;
  let disposed = false;
  const armTimer = (timer: TimerEntry, isActive: () => boolean, retire: () => void): void => {
    const delay = Math.min(timer.remainingMs, MAX_HOST_TIMEOUT_MS);
    timer.handle = setTimeoutHost(() => {
      if (!isActive()) return;
      timer.remainingMs -= delay;
      if (timer.remainingMs > 0) {
        try {
          armTimer(timer, isActive, retire);
        } catch (error) {
          retire();
          timer.reject(error);
        }
        return;
      }
      retire();
      timer.resolve();
    }, delay);
  };
  const cancelTimer = (timer: TimerEntry): void => {
    try {
      if (timer.handle !== undefined) {
        clearTimeoutHost(timer.handle);
      }
    } catch {
      // Rust invokes this callback through a non-catching TSFN. Contain host
      // cancellation failures at the JavaScript boundary.
    } finally {
      // Rust awaits the schedule promise. Resolve even if the host cancellation
      // API throws so the detached relay can still retire.
      timer.resolve();
    }
  };
  const schedule = (idOrMs: number, ms?: number): Promise<void> => {
    if (disposed) return Promise.resolve();
    // Compatibility with the old `(ms) => Promise<void>` binding contract.
    if (ms === undefined) {
      return new Promise((resolve, reject) => {
        const entry: TimerEntry = {
          handle: undefined,
          remainingMs: Math.max(idOrMs, 0),
          reject,
          resolve,
        };
        legacyActive.add(entry);
        try {
          armTimer(
            entry,
            () => legacyActive.has(entry),
            () => legacyActive.delete(entry),
          );
        } catch (error) {
          legacyActive.delete(entry);
          reject(error);
        }
      });
    }

    const id = idOrMs;
    const previous = active.get(id);
    if (previous) {
      active.delete(id);
      cancelTimer(previous);
    }

    return new Promise((resolve, reject) => {
      const timer: TimerEntry = {
        handle: undefined,
        remainingMs: Math.max(ms, 0),
        reject,
        resolve,
      };
      active.set(id, timer);
      try {
        armTimer(
          timer,
          () => active.get(id) === timer,
          () => {
            active.delete(id);
          },
        );
      } catch (error) {
        if (active.get(id) === timer) {
          active.delete(id);
        }
        reject(error);
      }
    });
  };
  const cancel = (id: number): void => {
    const timer = active.get(id);
    if (!timer) return;
    active.delete(id);
    cancelTimer(timer);
  };
  const dispose = (): void => {
    if (disposed) return;
    if (registration) {
      Reflect.apply(unregisterTimerHost, binding, registration);
      registration = undefined;
    }
    disposed = true;
    const timers = [...active.values(), ...legacyActive];
    active.clear();
    legacyActive.clear();
    for (const timer of timers) {
      cancelTimer(timer);
    }
  };

  try {
    const reserved = Reflect.apply(reserveCurrentThreadHostRegistration, binding, []);
    let high: unknown;
    let low: unknown;
    try {
      if (reserved === null || (typeof reserved !== 'object' && typeof reserved !== 'function')) {
        throw new TypeError();
      }
      high = Reflect.get(reserved, 'high', reserved);
      low = Reflect.get(reserved, 'low', reserved);
    } catch {}
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
      throw new TypeError('The managed workerd binding returned an invalid host registration');
    }
    registration = [high, low];
    Reflect.apply(binding.registerTimerHost, binding, [...registration, schedule, cancel]);
    const active = Reflect.apply(isCurrentThreadHostRegistrationActive, binding, registration);
    if (typeof active !== 'boolean') {
      throw new TypeError(
        'The managed workerd binding returned an invalid timer host liveness result',
      );
    }
    if (!active) {
      throw new TypeError(
        'The managed workerd binding returned an inactive timer host registration',
      );
    }
  } catch (error) {
    try {
      dispose();
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        'Managed workerd timer-host setup failed and rollback did not complete',
        { cause: error },
      );
    }
    throw error;
  }
  return dispose;
}
