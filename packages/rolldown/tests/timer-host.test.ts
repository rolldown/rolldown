// @ts-nocheck This focused unit test mocks the generated binding surface.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const callbacks = vi.hoisted(() => ({
  schedule: undefined as undefined | ((id: number, ms: number) => Promise<void>),
  cancel: undefined as undefined | ((id: number) => void),
}));
const bindingState = vi.hoisted(() => ({
  activeRegistrations: new Set<string>(),
  asyncRuntimeBuild: true,
  backend: 'shared',
  flavor: 'CurrentThread',
  nextRegistrationLow: 1,
  reservedRegistrations: new Set<string>(),
  version: 4,
}));
const hostInstallationsKey = Symbol.for('rolldown.current-thread-host-installations.v4');

function reserveRegistration(low: number) {
  const registration = { high: 0, low };
  bindingState.reservedRegistrations.add(`0:${low}`);
  return registration;
}

vi.mock('../src/binding.cjs', () => ({
  __rolldownBindingTarget: 'native',
  getRuntimeCapabilities: vi.fn(() => ({
    asyncRuntimeBuild: bindingState.asyncRuntimeBuild,
    backend: bindingState.backend,
    blockOnJsThreadSafe: false,
    devSupported: bindingState.flavor === 'MultiThread',
    flavor: bindingState.flavor,
    target: 'native',
    threads: bindingState.flavor === 'MultiThread',
    timers: bindingState.flavor === 'MultiThread',
    wasi: false,
    watchSupported: true,
  })),
  getCurrentThreadTaskHostContractVersion: vi.fn(() => bindingState.version),
  isCurrentThreadHostRegistrationActive: vi.fn((high: number, low: number) =>
    bindingState.activeRegistrations.has(`${high}:${low}`),
  ),
  reserveCurrentThreadHostRegistration: vi.fn(() => {
    const registration = { high: 0, low: bindingState.nextRegistrationLow++ };
    bindingState.reservedRegistrations.add(`${registration.high}:${registration.low}`);
    return registration;
  }),
  registerCurrentThreadTaskHost: vi.fn((high: number, low: number) => {
    const key = `${high}:${low}`;
    if (!bindingState.reservedRegistrations.delete(key)) {
      throw new TypeError('task-host registration was not reserved');
    }
    bindingState.activeRegistrations.add(key);
  }),
  registerTimerHost: vi.fn(
    (
      high: number,
      low: number,
      schedule: (id: number, ms: number) => Promise<void>,
      cancel: (id: number) => void,
    ) => {
      const key = `${high}:${low}`;
      if (!bindingState.reservedRegistrations.delete(key)) {
        throw new TypeError('timer-host registration was not reserved');
      }
      callbacks.schedule = schedule;
      callbacks.cancel = cancel;
      bindingState.activeRegistrations.add(key);
    },
  ),
  unregisterCurrentThreadTaskHost: vi.fn((high: number, low: number) => {
    const key = `${high}:${low}`;
    bindingState.reservedRegistrations.delete(key);
    bindingState.activeRegistrations.delete(key);
  }),
  unregisterTimerHost: vi.fn((high: number, low: number) => {
    const key = `${high}:${low}`;
    bindingState.reservedRegistrations.delete(key);
    bindingState.activeRegistrations.delete(key);
  }),
}));

beforeEach(async () => {
  vi.resetModules();
  Reflect.deleteProperty(globalThis, hostInstallationsKey);
  bindingState.asyncRuntimeBuild = true;
  bindingState.backend = 'shared';
  bindingState.flavor = 'CurrentThread';
  bindingState.version = 4;
  bindingState.nextRegistrationLow = 1;
  bindingState.activeRegistrations.clear();
  bindingState.reservedRegistrations.clear();
  callbacks.schedule = undefined;
  callbacks.cancel = undefined;
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.getRuntimeCapabilities).mockClear();
  vi.mocked(binding.getCurrentThreadTaskHostContractVersion).mockClear();
  vi.mocked(binding.isCurrentThreadHostRegistrationActive).mockClear();
  vi.mocked(binding.reserveCurrentThreadHostRegistration).mockClear();
  vi.mocked(binding.registerCurrentThreadTaskHost).mockClear();
  vi.mocked(binding.registerTimerHost).mockClear();
  vi.mocked(binding.unregisterCurrentThreadTaskHost).mockClear();
  vi.mocked(binding.unregisterTimerHost).mockClear();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  Reflect.deleteProperty(globalThis, hostInstallationsKey);
});

test('CurrentThread task host installs the native driver without a JavaScript callback', async () => {
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');
  const { registerCurrentThreadTaskHost } = await import('../src/binding.cjs');

  expect(registerCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
});

test('Tokio runtimes do not install shared-runtime hosts', async () => {
  bindingState.asyncRuntimeBuild = false;
  bindingState.backend = 'tokio';
  bindingState.flavor = 'MultiThread';

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).resolves.toBeDefined();
  const binding = await import('../src/binding.cjs');

  expect(binding.getRuntimeCapabilities).toHaveBeenCalledOnce();
  expect(binding.getCurrentThreadTaskHostContractVersion).not.toHaveBeenCalled();
  expect(binding.reserveCurrentThreadHostRegistration).not.toHaveBeenCalled();
  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('shared MultiThread runtimes proactively install hosts for a later flavor switch', async () => {
  bindingState.flavor = 'MultiThread';

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).resolves.toBeDefined();
  const binding = await import('../src/binding.cjs');

  expect(binding.getRuntimeCapabilities).toHaveBeenCalledOnce();
  expect(binding.getCurrentThreadTaskHostContractVersion).toHaveBeenCalledOnce();
  expect(binding.reserveCurrentThreadHostRegistration).toHaveBeenCalledTimes(2);
  expect(binding.registerCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
  expect(binding.registerTimerHost).toHaveBeenCalledOnce();
});

test('cache-busted evaluations reuse the same per-binding host registrations', async () => {
  // @ts-ignore The query forces a second production module evaluation.
  await import('../src/timer-host?host-copy=one');
  // @ts-ignore The query forces a third production module evaluation.
  await import('../src/timer-host?host-copy=two');
  const binding = await import('../src/binding.cjs');

  expect(binding.getCurrentThreadTaskHostContractVersion).toHaveBeenCalledTimes(2);
  expect(binding.registerCurrentThreadTaskHost).toHaveBeenCalledOnce();
  expect(binding.registerTimerHost).toHaveBeenCalledOnce();
});

test('host installation continues when the realm-global deduplication slot is unavailable', async () => {
  const realGlobal = globalThis;
  const spoofedRegistry = new Proxy(new WeakMap(), {});
  const blockedGlobal = new Proxy(realGlobal, {
    defineProperty(target, key, descriptor) {
      if (key === hostInstallationsKey) return false;
      return Reflect.defineProperty(target, key, descriptor);
    },
    get(target, key) {
      if (key === hostInstallationsKey) return spoofedRegistry;
      return Reflect.get(target, key, target);
    },
  });
  vi.stubGlobal('globalThis', blockedGlobal);

  // @ts-ignore The query forces an independent production module evaluation.
  await expect(import('../src/timer-host?blocked-host-cache=one')).resolves.toBeDefined();
  // @ts-ignore A second module has its own safe fallback cache.
  await expect(import('../src/timer-host?blocked-host-cache=two')).resolves.toBeDefined();
  const binding = await import('../src/binding.cjs');

  expect(binding.registerCurrentThreadTaskHost).toHaveBeenCalledTimes(2);
  expect(binding.registerTimerHost).toHaveBeenCalledTimes(2);
});

test('cache-busted evaluations replace native-evicted host registrations exactly once', async () => {
  // @ts-ignore The query forces a production module evaluation.
  await import('../src/timer-host?host-eviction=one');
  const binding = await import('../src/binding.cjs');
  binding.unregisterCurrentThreadTaskHost(0, 1);
  binding.unregisterTimerHost(0, 2);

  // @ts-ignore The query re-evaluates the same production registration path.
  await import('../src/timer-host?host-eviction=two');
  // @ts-ignore A third evaluation must observe the replacement as live.
  await import('../src/timer-host?host-eviction=three');

  expect(binding.registerCurrentThreadTaskHost).toHaveBeenCalledTimes(2);
  expect(binding.registerTimerHost).toHaveBeenCalledTimes(2);
  expect(binding.isCurrentThreadHostRegistrationActive).toHaveBeenCalledWith(0, 1);
  expect(binding.isCurrentThreadHostRegistrationActive).toHaveBeenCalledWith(0, 2);
  expect(binding.isCurrentThreadHostRegistrationActive).toHaveBeenCalledWith(0, 3);
  expect(binding.isCurrentThreadHostRegistrationActive).toHaveBeenCalledWith(0, 4);
});

test('CurrentThread task host rejects a malformed v4 reservation before installation', async () => {
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.reserveCurrentThreadHostRegistration).mockReturnValueOnce(undefined as never);

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /invalid CurrentThread task-host registration for contract version 4/,
    ),
  });
  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('CurrentThread task host rejects the reserved zero registration', async () => {
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.reserveCurrentThreadHostRegistration).mockReturnValueOnce({
    high: 0,
    low: 0,
  });

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /invalid CurrentThread task-host registration for contract version 4/,
    ),
  });
  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('CurrentThread task host rolls back an inactive registration', async () => {
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.registerCurrentThreadTaskHost).mockImplementationOnce((high, low) => {
    bindingState.reservedRegistrations.delete(`${high}:${low}`);
  });

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(/inactive CurrentThread task-host registration/),
  });
  expect(binding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('CurrentThread task host is rolled back when timer registration fails', async () => {
  const binding = await import('../src/binding.cjs');
  const timerError = new Error('timer registration failed');
  vi.mocked(binding.registerTimerHost).mockImplementationOnce(() => {
    throw timerError;
  });

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toBe(timerError);
  expect(binding.unregisterTimerHost).toHaveBeenCalledWith(0, 2);
  expect(binding.unregisterCurrentThreadTaskHost).toHaveBeenCalledOnce();
  expect(binding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
});

test('CurrentThread task-host rollback preserves timer and cleanup failures', async () => {
  const binding = await import('../src/binding.cjs');
  const timerError = new Error('timer registration failed');
  const cleanupError = new Error('task host rollback failed');
  vi.mocked(binding.registerTimerHost).mockImplementationOnce(() => {
    throw timerError;
  });
  vi.mocked(binding.unregisterCurrentThreadTaskHost).mockImplementationOnce(() => {
    throw cleanupError;
  });

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  const error = await import('../src/timer-host').catch((error: unknown) => error);
  expect(error).toMatchObject({
    cause: timerError,
    errors: [timerError, cleanupError],
  });
  expect(binding.unregisterTimerHost).toHaveBeenCalledWith(0, 2);
});

test('CurrentThread timer host rejects a malformed v4 reservation before installation', async () => {
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.reserveCurrentThreadHostRegistration)
    .mockImplementationOnce(() => reserveRegistration(1))
    .mockReturnValueOnce(undefined as never);

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /invalid CurrentThread timer-host registration for contract version 4/,
    ),
  });
  expect(binding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('CurrentThread timer host rejects the reserved zero registration', async () => {
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.reserveCurrentThreadHostRegistration)
    .mockImplementationOnce(() => reserveRegistration(1))
    .mockReturnValueOnce({ high: 0, low: 0 });

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /invalid CurrentThread timer-host registration for contract version 4/,
    ),
  });
  expect(binding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('CurrentThread timer host rolls back an inactive registration', async () => {
  const binding = await import('../src/binding.cjs');
  vi.mocked(binding.reserveCurrentThreadHostRegistration)
    .mockImplementationOnce(() => reserveRegistration(1))
    .mockImplementationOnce(() => reserveRegistration(42));
  vi.mocked(binding.registerTimerHost).mockImplementationOnce((high, low) => {
    bindingState.reservedRegistrations.delete(`${high}:${low}`);
  });

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(/inactive CurrentThread timer-host registration/),
  });
  expect(binding.unregisterTimerHost).toHaveBeenCalledWith(0, 42);
  expect(binding.unregisterCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
});

test('CurrentThread setup preserves mismatch and rollback failures with poisoned AggregateError', async () => {
  const binding = await import('../src/binding.cjs');
  const timerCleanupError = new Error('timer host rollback failed');
  const taskCleanupError = new Error('task host rollback failed');
  vi.mocked(binding.reserveCurrentThreadHostRegistration)
    .mockImplementationOnce(() => reserveRegistration(1))
    .mockImplementationOnce(() => reserveRegistration(43));
  vi.mocked(binding.registerTimerHost).mockImplementationOnce((high, low) => {
    bindingState.reservedRegistrations.delete(`${high}:${low}`);
  });
  vi.mocked(binding.unregisterTimerHost).mockImplementationOnce(() => {
    throw timerCleanupError;
  });
  vi.mocked(binding.unregisterCurrentThreadTaskHost).mockImplementationOnce(() => {
    throw taskCleanupError;
  });
  class ThrowingAggregateError {
    constructor() {
      throw new Error('poisoned AggregateError');
    }
  }
  vi.stubGlobal('AggregateError', ThrowingAggregateError);

  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  const error = await import('../src/timer-host').catch((error: unknown) => error);
  expect(error).toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    errors: [
      expect.objectContaining({
        code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
        message: expect.stringMatching(/inactive CurrentThread timer-host registration/),
      }),
      timerCleanupError,
      taskCleanupError,
    ],
  });
  expect((error as { cause: unknown }).cause).toBe((error as { errors: unknown[] }).errors[0]);
});

test('CurrentThread host cancellation clears the JS timeout and resolves its relay', async () => {
  vi.useFakeTimers();
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const relay = callbacks.schedule?.(7, 60_000);
  expect(relay).toBeDefined();
  expect(vi.getTimerCount()).toBe(1);

  callbacks.cancel?.(7);

  await expect(relay).resolves.toBeUndefined();
  expect(vi.getTimerCount()).toBe(0);
});

test('CurrentThread host splits delays above the Node timeout limit', async () => {
  vi.useFakeTimers();
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const maxHostTimeoutMs = 2_147_483_647;
  const relay = callbacks.schedule?.(8, maxHostTimeoutMs + 25);
  expect(relay).toBeDefined();

  let settled = false;
  void relay?.then(() => {
    settled = true;
  });

  await vi.advanceTimersByTimeAsync(maxHostTimeoutMs);
  expect(settled).toBe(false);
  expect(vi.getTimerCount()).toBe(1);

  await vi.advanceTimersByTimeAsync(24);
  expect(settled).toBe(false);

  await vi.advanceTimersByTimeAsync(1);
  expect(settled).toBe(true);
  expect(vi.getTimerCount()).toBe(0);
});

test('CurrentThread host rejects its relay when a chained timer cannot be armed', async () => {
  vi.useFakeTimers();
  const maxHostTimeoutMs = 2_147_483_647;
  const error = new Error('setTimeout failed');
  const originalSetTimeout = globalThis.setTimeout;
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  let setTimeoutCalls = 0;
  vi.spyOn(globalThis, 'setTimeout').mockImplementation((handler, timeout, ...args) => {
    setTimeoutCalls += 1;
    if (setTimeoutCalls === 2) {
      throw error;
    }
    return originalSetTimeout(handler, timeout, ...args);
  });

  const relay = callbacks.schedule?.(9, maxHostTimeoutMs + 1);

  const rejection = expect(relay).rejects.toBe(error);
  await vi.advanceTimersByTimeAsync(maxHostTimeoutMs);

  await rejection;
});

test('CurrentThread host captures replacement timer APIs when scheduling', async () => {
  vi.useFakeTimers();
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');
  vi.useRealTimers();

  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const setTimeoutReplacement = vi
    .spyOn(globalThis, 'setTimeout')
    .mockImplementation((handler, timeout, ...args) =>
      originalSetTimeout(handler, timeout, ...args),
    );
  const clearTimeoutReplacement = vi
    .spyOn(globalThis, 'clearTimeout')
    .mockImplementation((handle) => originalClearTimeout(handle));
  const relay = callbacks.schedule?.(10, 60_000);
  expect(setTimeoutReplacement).toHaveBeenCalledOnce();
  const handle = setTimeoutReplacement.mock.results[0]?.value;

  expect(() => callbacks.cancel?.(10)).not.toThrow();
  expect(clearTimeoutReplacement).toHaveBeenCalledWith(handle);
  await expect(relay).resolves.toBeUndefined();
});

test('CurrentThread host retains schedule-time timer APIs across chunks and cancellation', async () => {
  vi.useFakeTimers();
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const maxHostTimeoutMs = 2_147_483_647;
  const relay = callbacks.schedule?.(11, maxHostTimeoutMs + 1);
  expect(vi.getTimerCount()).toBe(1);
  const setTimeoutReplacement = vi.spyOn(globalThis, 'setTimeout').mockImplementation(() => {
    throw new Error('replacement setTimeout should not be used');
  });
  const clearTimeoutReplacement = vi
    .spyOn(globalThis, 'clearTimeout')
    .mockImplementation(() => undefined);

  await vi.advanceTimersByTimeAsync(maxHostTimeoutMs);
  expect(setTimeoutReplacement).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(1);

  expect(() => callbacks.cancel?.(11)).not.toThrow();
  expect(clearTimeoutReplacement).not.toHaveBeenCalled();
  await expect(relay).resolves.toBeUndefined();
  expect(vi.getTimerCount()).toBe(0);
});

test('CurrentThread host settles its relay when the captured cancellation throws', async () => {
  vi.useFakeTimers();
  const originalClearTimeout = globalThis.clearTimeout;
  const error = new Error('clearTimeout failed');
  const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation((handle) => {
    originalClearTimeout(handle);
    throw error;
  });
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const relay = callbacks.schedule?.(12, 60_000);
  expect(vi.getTimerCount()).toBe(1);

  expect(() => callbacks.cancel?.(12)).not.toThrow();
  await expect(relay).resolves.toBeUndefined();
  expect(vi.getTimerCount()).toBe(0);
  expect(reported).toHaveBeenCalledOnce();
});

test('CurrentThread host cancels a long timeout when clearTimeout throws before delegation', async () => {
  vi.useFakeTimers();
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const clearError = new Error('clearTimeout failed before delegation');
  const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  const close = vi.fn();
  vi.spyOn(globalThis, 'setTimeout').mockImplementation((handler, timeout, ...args) => {
    const innerHandle = originalSetTimeout(handler, timeout, ...args);
    return {
      close: () => {
        close();
        originalClearTimeout(innerHandle);
      },
      unref: vi.fn(),
    } as unknown as ReturnType<typeof setTimeout>;
  });
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation(() => {
    throw clearError;
  });
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const relay = callbacks.schedule?.(13, 2_147_483_647);
  expect(vi.getTimerCount()).toBe(1);

  expect(() => callbacks.cancel?.(13)).not.toThrow();
  await expect(relay).resolves.toBeUndefined();
  expect(close).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
  expect(reported).toHaveBeenCalledWith(
    expect.objectContaining({
      cause: clearError,
      message: expect.stringContaining('timeout.close()'),
    }),
  );
});

test('CurrentThread host preserves the unreferenced outcome when AggregateError is poisoned', async () => {
  vi.useFakeTimers();
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const clearError = new Error('clearTimeout failed');
  const closeError = new Error('timeout.close failed');
  const unref = vi.fn();
  const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  let innerHandle: ReturnType<typeof setTimeout> | undefined;
  vi.spyOn(globalThis, 'setTimeout').mockImplementation((handler, timeout, ...args) => {
    innerHandle = originalSetTimeout(handler, timeout, ...args);
    return {
      close: () => {
        throw closeError;
      },
      unref,
    } as unknown as ReturnType<typeof setTimeout>;
  });
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation(() => {
    throw clearError;
  });
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');
  class ThrowingAggregateError {
    constructor() {
      throw new Error('poisoned AggregateError');
    }
  }
  vi.stubGlobal('AggregateError', ThrowingAggregateError);

  const relay = callbacks.schedule?.(14, 2_147_483_647);
  expect(() => callbacks.cancel?.(14)).not.toThrow();
  await expect(relay).resolves.toBeUndefined();

  expect(unref).toHaveBeenCalledOnce();
  expect(reported).toHaveBeenCalledWith(
    expect.objectContaining({
      cause: clearError,
      errors: [clearError, closeError],
      message: expect.stringContaining('unreferenced and may still fire'),
    }),
  );
  if (innerHandle !== undefined) {
    originalClearTimeout(innerHandle);
  }
});

test('CurrentThread host rejects its relay when cancellation cannot cancel or unreference', async () => {
  vi.useFakeTimers();
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const clearError = new Error('clearTimeout failed');
  const closeError = new Error('timeout.close failed');
  const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  let innerHandle: ReturnType<typeof setTimeout> | undefined;
  vi.spyOn(globalThis, 'setTimeout').mockImplementation((handler, timeout, ...args) => {
    innerHandle = originalSetTimeout(handler, timeout, ...args);
    return {
      close: () => {
        throw closeError;
      },
    } as unknown as ReturnType<typeof setTimeout>;
  });
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation(() => {
    throw clearError;
  });
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const relay = callbacks.schedule?.(15, 2_147_483_647);
  expect(() => callbacks.cancel?.(15)).toThrow(
    expect.objectContaining({
      cause: clearError,
      errors: [clearError, closeError],
      message: expect.stringContaining('could not be cancelled or unreferenced'),
    }),
  );
  await expect(relay).rejects.toMatchObject({
    cause: clearError,
    errors: [clearError, closeError],
    message: expect.stringContaining('could not be cancelled or unreferenced'),
  });
  expect(reported).toHaveBeenCalledOnce();

  if (innerHandle !== undefined) {
    originalClearTimeout(innerHandle);
  }
});

test('CurrentThread host preserves cancellation failures when AggregateError throws', async () => {
  vi.useFakeTimers();
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const clearError = new Error('clearTimeout failed');
  const closeError = new Error('timeout.close failed');
  const diagnosticError = new Error('AggregateError construction failed');
  const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  let innerHandle: ReturnType<typeof setTimeout> | undefined;
  vi.spyOn(globalThis, 'setTimeout').mockImplementation((handler, timeout, ...args) => {
    innerHandle = originalSetTimeout(handler, timeout, ...args);
    return {
      close: () => {
        throw closeError;
      },
    } as unknown as ReturnType<typeof setTimeout>;
  });
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation(() => {
    throw clearError;
  });
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');
  class ThrowingAggregateError {
    constructor() {
      throw diagnosticError;
    }
  }
  vi.stubGlobal('AggregateError', ThrowingAggregateError);

  const relay = callbacks.schedule?.(16, 2_147_483_647);
  expect(() => callbacks.cancel?.(16)).toThrow(
    expect.objectContaining({
      cause: clearError,
      errors: [clearError, closeError],
      message: expect.stringContaining('could not be cancelled or unreferenced'),
    }),
  );
  await expect(relay).rejects.toMatchObject({
    cause: clearError,
    errors: [clearError, closeError],
    message: expect.stringContaining('could not be cancelled or unreferenced'),
  });
  expect(reported).toHaveBeenCalledWith(
    expect.objectContaining({
      cause: clearError,
      errors: [clearError, closeError],
    }),
  );
  expect(reported).not.toHaveBeenCalledWith(diagnosticError);

  if (innerHandle !== undefined) {
    originalClearTimeout(innerHandle);
  }
});

test('CurrentThread host settles cancellation when AggregateError is not constructible', async () => {
  vi.useFakeTimers();
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;
  const clearError = new Error('clearTimeout failed');
  const closeError = new Error('timeout.close failed');
  const reported = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  let innerHandle: ReturnType<typeof setTimeout> | undefined;
  vi.spyOn(globalThis, 'setTimeout').mockImplementation((handler, timeout, ...args) => {
    innerHandle = originalSetTimeout(handler, timeout, ...args);
    return {
      close: () => {
        throw closeError;
      },
    } as unknown as ReturnType<typeof setTimeout>;
  });
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation(() => {
    throw clearError;
  });
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');
  vi.stubGlobal('AggregateError', () => undefined);

  const relay = callbacks.schedule?.(17, 2_147_483_647);
  expect(() => callbacks.cancel?.(17)).toThrow(
    expect.objectContaining({
      cause: clearError,
      errors: [clearError, closeError],
      message: expect.stringContaining('could not be cancelled or unreferenced'),
    }),
  );
  await expect(relay).rejects.toMatchObject({
    cause: clearError,
    errors: [clearError, closeError],
    message: expect.stringContaining('could not be cancelled or unreferenced'),
  });
  expect(reported).toHaveBeenCalledWith(
    expect.objectContaining({
      cause: clearError,
      errors: [clearError, closeError],
    }),
  );

  if (innerHandle !== undefined) {
    originalClearTimeout(innerHandle);
  }
});
