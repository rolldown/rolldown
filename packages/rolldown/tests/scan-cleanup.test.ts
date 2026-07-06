import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingConstructions: 0,
  callOptionsHook: vi.fn(async (option) => option),
  close: vi.fn(),
  createBundlerOptions: vi.fn(),
  pluginPromiseThenCalls: 0,
  runtimeCapabilities: {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  },
  scan: vi.fn(),
}));

vi.mock('../src/binding.cjs', () => ({
  BindingBundler: class {
    constructor() {
      mocks.bindingConstructions += 1;
    }
    close = mocks.close;
    scan = mocks.scan;
  },
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: mocks.callOptionsHook,
  },
}));

vi.mock('../src/runtime-lifecycle', () => ({
  acquireRuntimeLease: mocks.acquireRuntimeLease,
  isRuntimeLeaseRequired: () => false,
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: mocks.createBundlerOptions,
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { scan } from '../src/api/experimental';
// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { getRetryableCleanup } from '../src/utils/retryable-cleanup';

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.bindingConstructions = 0;
  mocks.callOptionsHook.mockClear();
  mocks.close.mockReset();
  mocks.createBundlerOptions.mockReset();
  mocks.pluginPromiseThenCalls = 0;
  mocks.scan.mockReset();
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  });
});

test('scan rejects output descriptors before input promises, hooks, or setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
  });
  let outputOptionsHookCalls = 0;

  await expect(
    scan(
      {
        plugins: [
          {
            // oxlint-disable-next-line unicorn/no-thenable -- verifies preflight before promise assimilation
            then() {
              mocks.pluginPromiseThenCalls += 1;
              return new Promise(() => {});
            },
          } as never,
          {
            name: 'input-options-side-effect',
            options() {
              throw new Error('input options hook must not run');
            },
          },
        ],
      },
      {
        plugins: [
          {
            name: 'output-options-side-effect',
            outputOptions(options) {
              outputOptionsHookCalls += 1;
              return options;
            },
          },
          {
            _parallel: {
              fileUrl: 'file:///project/old-package-plugin.mjs',
              options: {},
            },
          } as never,
        ],
      },
    ),
  ).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });

  expect(mocks.pluginPromiseThenCalls).toBe(0);
  expect(outputOptionsHookCalls).toBe(0);
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});

test('scan setup retries parallel-worker cleanup after the first termination rejection', async () => {
  const setupError = new Error('runtime lease setup failed');
  const cleanupError = new Error('worker termination failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue({
    bundlerOptions: {},
    inputOptions: { input: 'entry.js' },
    onLog: vi.fn(),
    stopWorkers,
  });
  mocks.acquireRuntimeLease.mockRejectedValue(setupError);

  const error = await scan({ input: 'entry.js' }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([setupError, cleanupError]);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('scan retry clears worker ownership even when native close remains failed', async () => {
  const scanError = new Error('scan failed');
  const nativeCloseError = new Error('native close failed');
  const cleanupError = new Error('worker termination failed');
  const release = vi.fn();
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue({
    bundlerOptions: {},
    inputOptions: { input: 'entry.js' },
    onLog: vi.fn(),
    stopWorkers,
  });
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.scan.mockRejectedValue(scanError);
  mocks.close.mockRejectedValue(nativeCloseError);

  const error = await scan({ input: 'entry.js' }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(scanError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(mocks.close).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(error)).toBeUndefined();
});
