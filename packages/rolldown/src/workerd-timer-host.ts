const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 2;

type HostRegistration = {
  high: number;
  low: number;
};

type TimerHostRegistration = (
  schedule: (idOrMs: number, ms?: number) => Promise<void>,
  cancel: (id: number) => void,
) => HostRegistration | undefined;

interface WorkerdTimerBinding {
  getCurrentThreadTaskHostContractVersion?: () => unknown;
  registerTimerHost: TimerHostRegistration;
  unregisterTimerHost?: (high: number, low: number) => void;
}

/**
 * Register the CurrentThread timer bridge for one managed workerd instance.
 *
 * The optional second schedule argument keeps this compatible with the
 * pre-cancellation binding, which invokes the callback as `(ms)`.
 */
export function registerWorkerdTimerHost(binding: WorkerdTimerBinding): () => void {
  const setTimeoutHost = globalThis.setTimeout?.bind(globalThis);
  const clearTimeoutHost = globalThis.clearTimeout?.bind(globalThis);
  if (!setTimeoutHost || !clearTimeoutHost) return () => {};

  const getCurrentThreadTaskHostContractVersion = Reflect.get(
    binding,
    'getCurrentThreadTaskHostContractVersion',
  );
  const unregisterTimerHost = Reflect.get(binding, 'unregisterTimerHost');
  const legacyHostContract =
    getCurrentThreadTaskHostContractVersion === undefined && unregisterTimerHost === undefined;
  let unregisterTimerHostFunction: ((high: number, low: number) => void) | undefined;
  if (
    !legacyHostContract &&
    (typeof getCurrentThreadTaskHostContractVersion !== 'function' ||
      typeof unregisterTimerHost !== 'function')
  ) {
    throw new TypeError('The managed workerd binding does not support exact timer-host disposal');
  }
  if (!legacyHostContract) {
    const getContractVersion = getCurrentThreadTaskHostContractVersion as () => unknown;
    unregisterTimerHostFunction = unregisterTimerHost as (high: number, low: number) => void;
    const actualVersion = Reflect.apply(getContractVersion, binding, []);
    if (actualVersion !== CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION) {
      throw new TypeError(
        `The managed workerd binding uses CurrentThread task-host contract version ` +
          `${String(actualVersion)}, but version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION} is required`,
      );
    }
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
      Reflect.apply(unregisterTimerHostFunction!, binding, registration);
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
    const result: unknown = Reflect.apply(binding.registerTimerHost, binding, [schedule, cancel]);
    if (!legacyHostContract) {
      let high: unknown;
      let low: unknown;
      try {
        if (result === null || (typeof result !== 'object' && typeof result !== 'function')) {
          throw new TypeError();
        }
        high = Reflect.get(result, 'high', result);
        low = Reflect.get(result, 'low', result);
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
    }
  } catch (error) {
    dispose();
    throw error;
  }
  return dispose;
}
