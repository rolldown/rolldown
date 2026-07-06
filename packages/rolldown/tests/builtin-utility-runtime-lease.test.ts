// @ts-nocheck This focused unit test mocks the generated binding surface.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  let resolveAcquisition;
  let rejectHook;
  let resolveHook;
  const release = vi.fn();
  const hook = vi.fn(
    () =>
      new Promise((resolve, reject) => {
        rejectHook = reject;
        resolveHook = resolve;
      }),
  );

  class BindingCallableBuiltinPlugin {
    transform = (...args) => hook(...args);

    getOrder() {
      return null;
    }
  }

  return {
    acquireAsyncRuntime: vi.fn(
      () =>
        new Promise((resolve) => {
          resolveAcquisition = resolve;
        }),
    ),
    BindingCallableBuiltinPlugin,
    completeAcquisition: () => resolveAcquisition({ release }),
    completeHook: (value) => resolveHook(value),
    failHook: (error) => rejectHook(error),
    getRuntimeCapabilities: () => ({ target: 'wasi-threads' }),
    hook,
    release,
  };
});

vi.mock('../src/binding.cjs', () => binding);

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { BuiltinPlugin, makeBuiltinPluginCallable } from '../src/builtin-plugin/utils';

beforeEach(() => {
  vi.clearAllMocks();
});

test('callable builtin async hooks acquire an outer runtime lease before entering native code', async () => {
  const plugin = makeBuiltinPluginCallable(new BuiltinPlugin('builtin:replace'));
  const operation = plugin.transform('source', 'input.js', {});

  await vi.waitFor(() => expect(binding.acquireAsyncRuntime).toHaveBeenCalledOnce());
  expect(binding.hook).not.toHaveBeenCalled();

  binding.completeAcquisition();
  await vi.waitFor(() => expect(binding.hook).toHaveBeenCalledOnce());
  expect(binding.release).not.toHaveBeenCalled();

  const result = { code: 'output' };
  binding.completeHook(result);
  await expect(operation).resolves.toBe(result);
  expect(binding.release).toHaveBeenCalledOnce();
});

test('callable builtin async hooks release the outer runtime lease after native rejection', async () => {
  const plugin = makeBuiltinPluginCallable(new BuiltinPlugin('builtin:replace'));
  const operation = plugin.transform('source', 'input.js', {});

  await vi.waitFor(() => expect(binding.acquireAsyncRuntime).toHaveBeenCalledOnce());
  binding.completeAcquisition();
  await vi.waitFor(() => expect(binding.hook).toHaveBeenCalledOnce());

  const error = new Error('hook failed');
  binding.failHook(error);
  await expect(operation).rejects.toBe(error);
  expect(binding.release).toHaveBeenCalledOnce();
});

test('callable builtin async hooks release the outer runtime lease after native setup failure', async () => {
  const error = new Error('hook setup failed');
  binding.hook.mockImplementationOnce(() => {
    throw error;
  });
  const plugin = makeBuiltinPluginCallable(new BuiltinPlugin('builtin:replace'));
  const operation = plugin.transform('source', 'input.js', {});

  await vi.waitFor(() => expect(binding.acquireAsyncRuntime).toHaveBeenCalledOnce());
  binding.completeAcquisition();

  await expect(operation).rejects.toBe(error);
  expect(binding.hook).toHaveBeenCalledOnce();
  expect(binding.release).toHaveBeenCalledOnce();
});
