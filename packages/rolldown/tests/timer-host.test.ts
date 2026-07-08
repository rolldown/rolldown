import { afterEach, expect, test, vi } from 'vitest';

const callbacks = vi.hoisted(() => ({
  schedule: undefined as undefined | ((id: number, ms: number) => Promise<void>),
  cancel: undefined as undefined | ((id: number) => void),
}));

vi.mock('../src/binding.cjs', () => ({
  getCurrentThreadTaskHostContractVersion: vi.fn(() => 1),
  registerCurrentThreadTaskHost: vi.fn(),
  registerTimerHost: vi.fn(
    (schedule: (id: number, ms: number) => Promise<void>, cancel: (id: number) => void) => {
      callbacks.schedule = schedule;
      callbacks.cancel = cancel;
    },
  ),
}));

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

test('CurrentThread task host installs the native driver without a JavaScript callback', async () => {
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');
  const { registerCurrentThreadTaskHost } = await import('../src/binding.cjs');

  expect(registerCurrentThreadTaskHost).toHaveBeenCalledWith();
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
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const maxHostTimeoutMs = 2_147_483_647;
  const relay = callbacks.schedule?.(9, maxHostTimeoutMs + 1);
  const error = new Error('setTimeout failed');
  vi.spyOn(globalThis, 'setTimeout').mockImplementation(() => {
    throw error;
  });

  const rejection = expect(relay).rejects.toBe(error);
  await vi.advanceTimersByTimeAsync(maxHostTimeoutMs);

  await rejection;
});

test('CurrentThread host settles its relay when cancellation throws', async () => {
  vi.useFakeTimers();
  // @ts-ignore The test intentionally imports package source outside the tests tsconfig root.
  await import('../src/timer-host');

  const relay = callbacks.schedule?.(10, 60_000);
  const error = new Error('clearTimeout failed');
  vi.spyOn(globalThis, 'clearTimeout').mockImplementation(() => {
    throw error;
  });

  expect(() => callbacks.cancel?.(10)).not.toThrow();
  await expect(relay).resolves.toBeUndefined();
});
