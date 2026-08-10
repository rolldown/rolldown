// @ts-nocheck This focused unit test mocks the generated binding surface.
import { beforeEach, expect, test, vi } from 'vitest';

// A pre-capability-reporter threaded-WASI binding: no getRuntimeCapabilities()
// export, so the compat shim synthesizes the legacy tokio-backed report from the
// generated loader target and requires a lease. Its implicit
// startAsyncRuntime()/shutdownAsyncRuntime() runtime-owner protocol cannot be
// coordinated across JavaScript realms and is never used.
const binding = vi.hoisted(() => ({
  shutdownAsyncRuntime: undefined as undefined | ReturnType<typeof vi.fn>,
  startAsyncRuntime: undefined as undefined | ReturnType<typeof vi.fn>,
}));

vi.mock('../src/binding.cjs', () => ({
  __rolldownBindingTarget: 'wasi-threads',
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
    message: expect.stringContaining('does not expose the acquireAsyncRuntime() lifecycle API'),
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
    message: expect.stringContaining('does not expose the acquireAsyncRuntime() lifecycle API'),
  });
});
