// @ts-nocheck This focused unit test mocks the generated binding surface.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const callbacks = vi.hoisted(() => ({
  schedule: undefined as undefined | ((id: number, ms: number) => Promise<void>),
  cancel: undefined as undefined | ((id: number) => void),
}));
const bindingState = vi.hoisted(() => ({
  activeRegistrations: new Set<string>(),
  nextRegistrationLow: 1,
  reservedRegistrations: new Set<string>(),
  version: 4,
}));
const hostInstallationsKey = Symbol.for('@napi-rs/async-runtime/current-thread-hosts/v4');

vi.mock('../src/binding.cjs', () => ({
  __napiBindingTarget: 'native',
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
  bindingState.version = 4;
  bindingState.nextRegistrationLow = 1;
  bindingState.activeRegistrations.clear();
  bindingState.reservedRegistrations.clear();
  callbacks.schedule = undefined;
  callbacks.cancel = undefined;
  const binding = await import('../src/binding.cjs');
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

test('both hosts are installed proactively for a later flavor switch', async () => {
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await expect(import('../src/timer-host')).resolves.toBeDefined();
  const binding = await import('../src/binding.cjs');

  expect(binding.getCurrentThreadTaskHostContractVersion).toHaveBeenCalledOnce();
  expect(binding.reserveCurrentThreadHostRegistration).toHaveBeenCalledTimes(2);
  expect(binding.registerCurrentThreadTaskHost).toHaveBeenCalledWith(0, 1);
  expect(binding.registerTimerHost).toHaveBeenCalledOnce();
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
