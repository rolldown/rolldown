import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingConstructions: 0,
  callOptionsHook: vi.fn(async (option) => option),
  close: vi.fn(),
  createBundlerOptions: vi.fn(),
  pluginPromiseThenCalls: 0,
  runtimeCapabilities: {
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
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
    closeTerminal = mocks.close;
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
import { getRetryableCleanup, recoverRetryableCleanups } from '../src/utils/retryable-cleanup';

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

test('scan setup-only cleanup remains eligible for abandoned recovery', async () => {
  const setupError = new Error('runtime lease setup failed');
  const firstCleanupError = new Error('first worker termination failed');
  const secondCleanupError = new Error('second worker termination failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(firstCleanupError)
    .mockRejectedValueOnce(secondCleanupError)
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
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(mocks.close).not.toHaveBeenCalled();
  expect(getRetryableCleanup(error)).toBeTypeOf('function');

  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(stopWorkers).toHaveBeenCalledTimes(3);
  expect(mocks.close).not.toHaveBeenCalled();
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
  mocks.close.mockResolvedValue(bindingErrors(nativeCloseError));

  const error = await scan({ input: 'entry.js' }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(scanError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(mocks.close).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('scan retries a synchronous native close transport failure before releasing ownership', async () => {
  const scanError = new Error('scan failed');
  const nativeCloseError = new Error('native close threw synchronously');
  const release = vi.fn();
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue({
    bundlerOptions: {},
    inputOptions: { input: 'entry.js' },
    onLog: vi.fn(),
    stopWorkers,
  });
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.scan.mockRejectedValue(scanError);
  mocks.close.mockImplementationOnce(() => {
    throw nativeCloseError;
  });
  mocks.close.mockResolvedValue(undefined);

  const error = await scan({ input: 'entry.js' }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([scanError, nativeCloseError]);
  expect(mocks.close).toHaveBeenCalledTimes(2);
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('scan retries an asynchronous native close transport rejection before cleanup', async () => {
  const nativeCloseError = new Error('native close transport rejected');
  const release = vi.fn();
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue({
    bundlerOptions: {},
    inputOptions: { input: 'entry.js' },
    onLog: vi.fn(),
    stopWorkers,
  });
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.scan.mockResolvedValue(undefined);
  mocks.close.mockRejectedValueOnce(nativeCloseError).mockResolvedValue(undefined);

  await expect(scan({ input: 'entry.js' })).rejects.toBe(nativeCloseError);

  expect(mocks.close).toHaveBeenCalledTimes(2);
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(nativeCloseError)).toBeUndefined();
});

test('scan preserves a terminal diagnostic delivered by a transport retry', async () => {
  const transportError = new Error('native close transport rejected');
  const terminalError = new Error('closeBundle failed after transport retry');
  const release = vi.fn();
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue({
    bundlerOptions: {},
    inputOptions: { input: 'entry.js' },
    onLog: vi.fn(),
    stopWorkers,
  });
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.scan.mockResolvedValue(undefined);
  mocks.close.mockRejectedValueOnce(transportError).mockResolvedValue(bindingErrors(terminalError));

  const error = await scan({ input: 'entry.js' }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([transportError, terminalError]);
  expect((error as AggregateError).cause).toBe(transportError);
  expect(mocks.close).toHaveBeenCalledTimes(2);
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(error)).toBeUndefined();
  expect(getRetryableCleanup(transportError)).toBeUndefined();
});

test(
  'scan awaits final native close retry outside nested setup recovery',
  { timeout: 5_000 },
  async () => {
    vi.useFakeTimers();
    try {
      const firstTransportError = new Error('first native close transport rejection');
      const secondTransportError = new Error('second native close transport rejection');
      const terminalError = new Error('closeBundle failed during final scan cleanup');
      const release = vi.fn();
      const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
      mocks.createBundlerOptions.mockResolvedValue({
        bundlerOptions: {},
        inputOptions: { input: 'entry.js' },
        onLog: vi.fn(),
        stopWorkers,
      });
      mocks.acquireRuntimeLease.mockResolvedValue({ release });
      mocks.scan.mockResolvedValue(undefined);
      mocks.close
        .mockRejectedValueOnce(firstTransportError)
        .mockRejectedValueOnce(secondTransportError)
        .mockImplementationOnce(async () => {
          await recoverRetryableCleanups();
          return bindingErrors(terminalError);
        });

      const operation = scan({ input: 'entry.js' });
      let settled = false;
      void operation.then(
        () => {
          settled = true;
        },
        () => {
          settled = true;
        },
      );
      await waitForCallCount(mocks.close, 2);

      expect(stopWorkers).not.toHaveBeenCalled();
      expect(release).not.toHaveBeenCalled();
      expect(settled).toBe(false);
      await waitForTimerCount(1);

      await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
      expect(mocks.close).toHaveBeenCalledTimes(2);

      await vi.runOnlyPendingTimersAsync();
      const scanError = await operation.catch((error: unknown) => error);

      expect(scanError).toBeInstanceOf(AggregateError);
      expect((scanError as AggregateError).errors).toEqual([
        firstTransportError,
        secondTransportError,
        terminalError,
      ]);
      expect(mocks.close).toHaveBeenCalledTimes(3);
      expect(stopWorkers).toHaveBeenCalledOnce();
      expect(release).toHaveBeenCalledOnce();
      expect(getRetryableCleanup(scanError)).toBeUndefined();
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  },
);

test('scan bounds abandoned recovery when native close persistently rejects', async () => {
  vi.useFakeTimers();
  try {
    const transportError = new Error('persistent native close transport rejection');
    const release = vi.fn();
    const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    mocks.createBundlerOptions.mockResolvedValue({
      bundlerOptions: {},
      inputOptions: { input: 'entry.js' },
      onLog: vi.fn(),
      stopWorkers,
    });
    mocks.acquireRuntimeLease.mockResolvedValue({ release });
    mocks.scan.mockResolvedValue(undefined);
    mocks.close.mockRejectedValue(transportError);

    const operation = scan({ input: 'entry.js' });
    const result = operation.catch((error: unknown) => error);
    await waitForCallCount(mocks.close, 2);
    expect(mocks.close).toHaveBeenCalledTimes(2);
    await waitForTimerCount(1);

    await vi.runOnlyPendingTimersAsync();
    const scanError = await result;
    const retryCleanup = getRetryableCleanup(scanError);

    expect(scanError).toBeInstanceOf(AggregateError);
    expect(retryCleanup).toBeTypeOf('function');
    expect(mocks.close).toHaveBeenCalledTimes(3);
    expect(vi.getTimerCount()).toBe(0);
    expect(getRetryableCleanup(scanError)).toBe(retryCleanup);
    expect(stopWorkers).not.toHaveBeenCalled();
    expect(release).not.toHaveBeenCalled();
  } finally {
    vi.useRealTimers();
  }
});

function bindingErrors(...errors: Error[]) {
  return {
    errors: errors.map((error) => ({ field0: error, type: 'JsError' })),
    isBindingErrors: true,
  };
}

async function waitForCallCount(
  mock: { mock: { calls: unknown[][] } },
  expectedCount: number,
): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (mock.mock.calls.length >= expectedCount) return;
    await Promise.resolve();
  }
  throw new Error(`Expected ${expectedCount} calls, received ${mock.mock.calls.length}`);
}

async function waitForTimerCount(expectedCount: number): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (vi.getTimerCount() >= expectedCount) return;
    await Promise.resolve();
  }
  throw new Error(`Expected ${expectedCount} timers, received ${vi.getTimerCount()}`);
}
