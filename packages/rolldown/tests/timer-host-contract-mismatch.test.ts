import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  getCurrentThreadTaskHostContractVersion: undefined as undefined | (() => unknown),
  registerCurrentThreadTaskHost: vi.fn((_dispatch?: unknown) => {}),
  registerTimerHost: vi.fn(),
}));

vi.mock('../src/binding.cjs', () => binding);

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  binding.getCurrentThreadTaskHostContractVersion = undefined;
});

test('rejects the previous callback-accepting binding before task-host invocation', async () => {
  await expect(import('../src/timer-host')).rejects.toThrow(
    /incomplete async-runtime host contract.*getCurrentThreadTaskHostContractVersion/,
  );

  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('rejects a mismatched native task-host contract version before registration', async () => {
  binding.getCurrentThreadTaskHostContractVersion = vi.fn(() => 0);

  await expect(import('../src/timer-host')).rejects.toThrow(
    /task-host contract version 0.*requires version 1/,
  );

  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});
