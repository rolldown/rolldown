const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION = 4;

/** Register and return an exact disposer for one native CurrentThread task host. */
export function registerWorkerdCurrentThreadTaskHost(binding: object): () => void {
  const getCurrentThreadTaskHostContractVersion = Reflect.get(
    binding,
    'getCurrentThreadTaskHostContractVersion',
  );
  const isCurrentThreadHostRegistrationActive = Reflect.get(
    binding,
    'isCurrentThreadHostRegistrationActive',
  );
  const registerCurrentThreadTaskHost = Reflect.get(binding, 'registerCurrentThreadTaskHost');
  const reserveCurrentThreadHostRegistration = Reflect.get(
    binding,
    'reserveCurrentThreadHostRegistration',
  );
  const unregisterCurrentThreadTaskHost = Reflect.get(binding, 'unregisterCurrentThreadTaskHost');
  if (
    typeof getCurrentThreadTaskHostContractVersion !== 'function' ||
    typeof isCurrentThreadHostRegistrationActive !== 'function' ||
    typeof registerCurrentThreadTaskHost !== 'function' ||
    typeof reserveCurrentThreadHostRegistration !== 'function' ||
    typeof unregisterCurrentThreadTaskHost !== 'function'
  ) {
    throw new TypeError('The managed workerd binding does not support CurrentThread task hosting');
  }

  const actualVersion = Reflect.apply(getCurrentThreadTaskHostContractVersion, binding, []);
  if (actualVersion !== CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION) {
    throw new TypeError(
      `The managed workerd binding uses CurrentThread task-host contract version ` +
        `${String(actualVersion)}, but version ${CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION} is required`,
    );
  }

  const registration: unknown = Reflect.apply(reserveCurrentThreadHostRegistration, binding, []);
  let high: unknown;
  let low: unknown;
  try {
    if (
      registration === null ||
      (typeof registration !== 'object' && typeof registration !== 'function')
    ) {
      throw new TypeError();
    }
    high = Reflect.get(registration, 'high', registration);
    low = Reflect.get(registration, 'low', registration);
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

  const rollback = (): void => {
    Reflect.apply(unregisterCurrentThreadTaskHost, binding, [high, low]);
  };
  try {
    Reflect.apply(registerCurrentThreadTaskHost, binding, [high, low]);
    const active = Reflect.apply(isCurrentThreadHostRegistrationActive, binding, [high, low]);
    if (typeof active !== 'boolean') {
      throw new TypeError(
        'The managed workerd binding returned an invalid task host liveness result',
      );
    }
    if (!active) {
      throw new TypeError(
        'The managed workerd binding returned an inactive task host registration',
      );
    }
  } catch (error) {
    try {
      rollback();
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        'Managed workerd task-host setup failed and rollback did not complete',
        { cause: error },
      );
    }
    throw error;
  }

  let disposed = false;
  return () => {
    if (disposed) return;
    rollback();
    disposed = true;
  };
}
