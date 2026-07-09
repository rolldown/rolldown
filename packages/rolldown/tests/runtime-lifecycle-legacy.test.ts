// @ts-nocheck This focused unit test mocks the generated binding surface.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  shutdownAsyncRuntime: undefined as undefined | ReturnType<typeof vi.fn>,
  startAsyncRuntime: undefined as undefined | ReturnType<typeof vi.fn>,
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
  get shutdownAsyncRuntime() {
    return binding.shutdownAsyncRuntime;
  },
  get startAsyncRuntime() {
    return binding.startAsyncRuntime;
  },
}));

beforeEach(() => {
  vi.resetModules();
  binding.shutdownAsyncRuntime = vi.fn();
  binding.startAsyncRuntime = vi.fn();
});

test('older threaded-WASI bindings fail closed instead of sharing implicit owners across realms', async () => {
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const { acquireRuntimeLease, isRuntimeLeaseRequired } = await import('../src/runtime-lifecycle');
  expect(isRuntimeLeaseRequired()).toBe(true);

  await expect(acquireRuntimeLease()).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining(
      'legacy implicit runtime-owner protocol, which cannot be coordinated safely across JavaScript realms',
    ),
  });
  expect(binding.startAsyncRuntime).not.toHaveBeenCalled();
  expect(binding.shutdownAsyncRuntime).not.toHaveBeenCalled();
});

test('threaded-WASI bindings without any lifecycle protocol fail with mismatch identity', async () => {
  binding.shutdownAsyncRuntime = undefined;
  binding.startAsyncRuntime = undefined;
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const { acquireRuntimeLease } = await import('../src/runtime-lifecycle');

  await expect(acquireRuntimeLease()).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining('does not expose acquireAsyncRuntime()'),
  });
});
