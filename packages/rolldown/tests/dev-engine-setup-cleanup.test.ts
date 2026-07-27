// @ts-nocheck These focused unit tests mock the generated binding surface.
import { afterEach, expect, test, vi } from 'vitest';

const NATIVE_SHARED_CAPABILITIES: Record<string, unknown> = {
  asyncRuntimeBuild: true,
  backend: 'shared',
  blockOnJsThreadSafe: false,
  devSupported: true,
  flavor: 'MultiThread',
  target: 'native',
  threads: true,
  timers: true,
  wasi: false,
  watchSupported: true,
};

// DevEngine.create() acquires its runtime lease AFTER createBundlerOptions has
// already spawned parallel-plugin workers, so every setup failure past that
// point must run `stopWorkers` -- mirroring the scan and watcher setup paths.
function mockBundlerOptions() {
  const stopWorkers = vi.fn().mockResolvedValue(undefined);
  vi.doMock('../src/utils/create-bundler-option', () => ({
    createBundlerOptions: vi.fn().mockResolvedValue({
      bundlerOptions: {},
      inputOptions: {},
      onLog: () => {},
      stopWorkers,
    }),
  }));
  return stopWorkers;
}

afterEach(() => {
  vi.doUnmock('../src/binding.cjs');
  vi.doUnmock('../src/utils/create-bundler-option');
  vi.resetModules();
});

test('a rejected runtime lease acquisition stops already-spawned workers', async () => {
  vi.resetModules();
  const engineConstructions = vi.fn();
  // A legacy threaded-WASI binding: no capability reporter, so the compat shim
  // synthesizes the tokio-backed report and requires a lease, and the missing
  // acquireAsyncRuntime() lifecycle API makes acquireRuntimeLease() reject.
  vi.doMock('../src/binding.cjs', () => ({
    __rolldownBindingTarget: 'wasi-threads',
    BindingDevEngine: class {
      constructor() {
        engineConstructions();
      }
    },
  }));
  const stopWorkers = mockBundlerOptions();

  const { DevEngine } = await import('../src/api/dev/dev-engine');
  await expect(DevEngine.create({ input: 'main.js' })).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining('acquireAsyncRuntime'),
  });

  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(engineConstructions).not.toHaveBeenCalled();
});

test('a failed BindingDevEngine construction stops workers and releases the lease', async () => {
  vi.resetModules();
  const constructionError = new Error('dev engine construction failed');
  vi.doMock('../src/binding.cjs', () => ({
    getRuntimeCapabilities: () => NATIVE_SHARED_CAPABILITIES,
    BindingDevEngine: class {
      constructor() {
        throw constructionError;
      }
    },
  }));
  const stopWorkers = mockBundlerOptions();

  const { DevEngine } = await import('../src/api/dev/dev-engine');
  await expect(DevEngine.create({ input: 'main.js' })).rejects.toBe(constructionError);

  expect(stopWorkers).toHaveBeenCalledOnce();
});

test('a failing worker stop surfaces together with the setup error', async () => {
  vi.resetModules();
  const constructionError = new Error('dev engine construction failed');
  const cleanupError = new Error('worker termination failed');
  vi.doMock('../src/binding.cjs', () => ({
    getRuntimeCapabilities: () => NATIVE_SHARED_CAPABILITIES,
    BindingDevEngine: class {
      constructor() {
        throw constructionError;
      }
    },
  }));
  const stopWorkers = vi.fn().mockRejectedValue(cleanupError);
  vi.doMock('../src/utils/create-bundler-option', () => ({
    createBundlerOptions: vi.fn().mockResolvedValue({
      bundlerOptions: {},
      inputOptions: {},
      onLog: () => {},
      stopWorkers,
    }),
  }));

  const { DevEngine } = await import('../src/api/dev/dev-engine');
  const failure = await DevEngine.create({ input: 'main.js' }).then(
    () => {
      throw new Error('DevEngine.create resolved despite the construction failure');
    },
    (error: unknown) => error,
  );

  expect(failure).toBeInstanceOf(AggregateError);
  expect(failure.errors).toEqual([constructionError, cleanupError]);
  expect(failure.cause).toBe(constructionError);
  expect(stopWorkers).toHaveBeenCalledOnce();
});

test('successful setup keeps the workers running', async () => {
  vi.resetModules();
  vi.doMock('../src/binding.cjs', () => ({
    getRuntimeCapabilities: () => NATIVE_SHARED_CAPABILITIES,
    BindingDevEngine: class {
      async close() {}
    },
  }));
  const stopWorkers = mockBundlerOptions();

  const { DevEngine } = await import('../src/api/dev/dev-engine');
  const engine = await DevEngine.create({ input: 'main.js' });
  expect(stopWorkers).not.toHaveBeenCalled();
  await engine.close();
});
