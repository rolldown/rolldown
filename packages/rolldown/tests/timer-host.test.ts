import { afterEach, expect, test, vi } from 'vitest';

const callbacks = vi.hoisted(() => ({
  schedule: undefined as undefined | ((id: number, ms: number) => Promise<void>),
  cancel: undefined as undefined | ((id: number) => void),
}));

vi.mock('../src/binding.cjs', () => ({
  registerTimerHost: vi.fn(
    (schedule: (id: number, ms: number) => Promise<void>, cancel: (id: number) => void) => {
      callbacks.schedule = schedule;
      callbacks.cancel = cancel;
    },
  ),
}));

afterEach(() => {
  vi.useRealTimers();
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
