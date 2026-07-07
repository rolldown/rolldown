// @ts-nocheck This focused unit test mocks the generated binding surface.
import { expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  shutdownAsyncRuntime: vi.fn(),
  startAsyncRuntime: vi.fn(),
}));

vi.mock('../src/binding.cjs', () => ({
  acquireAsyncRuntime: undefined,
  getRuntimeCapabilities: () => ({
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    flavor: 'MultiThread',
    target: 'wasi-threads',
    threads: true,
    timers: true,
    wasi: true,
    watchSupported: false,
  }),
  shutdownAsyncRuntime: binding.shutdownAsyncRuntime,
  startAsyncRuntime: binding.startAsyncRuntime,
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { acquireRuntimeLease, isRuntimeLeaseRequired } from '../src/runtime-lifecycle';

test('older threaded-WASI bindings fail closed instead of sharing implicit owners across realms', async () => {
  expect(isRuntimeLeaseRequired()).toBe(true);

  await expect(acquireRuntimeLease()).rejects.toThrow(
    'legacy implicit runtime-owner protocol, which cannot be coordinated safely across JavaScript realms',
  );
  expect(binding.startAsyncRuntime).not.toHaveBeenCalled();
  expect(binding.shutdownAsyncRuntime).not.toHaveBeenCalled();
});
