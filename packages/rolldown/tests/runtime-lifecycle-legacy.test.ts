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

test('older threaded-WASI bindings retain their implicit-owner lease protocol', async () => {
  expect(isRuntimeLeaseRequired()).toBe(true);

  const first = await acquireRuntimeLease();
  expect(binding.startAsyncRuntime).not.toHaveBeenCalled();

  const second = await acquireRuntimeLease();
  expect(binding.startAsyncRuntime).toHaveBeenCalledOnce();

  first.release();
  second.release();
  expect(binding.shutdownAsyncRuntime).toHaveBeenCalledTimes(2);
});
