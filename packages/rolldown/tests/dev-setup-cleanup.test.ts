// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingCloseTerminal: vi.fn(),
  bindingConstructionError: undefined as unknown,
  bindingConstructions: 0,
  callOptionsHook: vi.fn(async (option) => option),
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
}));

vi.mock('../src/binding.cjs', () => ({
  BindingDevEngine: class {
    constructor() {
      mocks.bindingConstructions += 1;
      if (mocks.bindingConstructionError) throw mocks.bindingConstructionError;
    }

    closeTerminal() {
      return mocks.bindingCloseTerminal();
    }
  },
  BindingRebuildStrategy: {
    Always: 'always',
    Auto: 'auto',
    Never: 'never',
  },
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: mocks.callOptionsHook,
  },
}));

vi.mock('../src/runtime-lifecycle', () => {
  const throwCloseErrors = (errors, aggregateMessage) => {
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, aggregateMessage, { cause: errors[0] });
    }
  };
  return {
    acquireRuntimeLease: mocks.acquireRuntimeLease,
    CloseCoordinator: class {
      constructor(aggregateMessage) {
        this.aggregateMessage = aggregateMessage;
      }

      close(attempt) {
        return (this.closePromise ??= Promise.resolve().then(async () => {
          const { errors, retryable } = await attempt();
          if (retryable) this.closePromise = undefined;
          throwCloseErrors(errors, this.aggregateMessage);
        }));
      }
    },
    throwCloseErrors,
  };
});

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: mocks.createBundlerOptions,
}));

import { DevEngine } from '../src/api/dev/dev-engine';
import {
  getRetryableCleanup,
  hasRetryableCleanupOwnership,
  recoverRetryableCleanups,
} from '../src/utils/retryable-cleanup';

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.bindingCloseTerminal.mockReset().mockResolvedValue(undefined);
  mocks.bindingConstructionError = undefined;
  mocks.bindingConstructions = 0;
  mocks.callOptionsHook.mockClear();
  mocks.createBundlerOptions.mockReset();
  mocks.pluginPromiseThenCalls = 0;
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  });
});

test('dev rejects descriptors before plugin promises or setup on threaded WASI', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
  });

  await expect(
    DevEngine.create(
      {
        plugins: [
          {
            // oxlint-disable-next-line unicorn/no-thenable -- verifies preflight before promise assimilation
            then() {
              mocks.pluginPromiseThenCalls += 1;
              return new Promise(() => {});
            },
          },
        ],
      },
      {
        plugins: [
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
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});

test('dev rejects CurrentThread before callbacks or setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: false,
    flavor: 'CurrentThread',
    threads: false,
  });
  const onOutput = vi.fn();

  await expect(DevEngine.create({}, {}, { onOutput })).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'dev',
  });

  expect(onOutput).not.toHaveBeenCalled();
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});

test('dev runtime setup retries failed worker cleanup', async () => {
  const setupError = new Error('runtime lease setup failed');
  const cleanupError = new Error('worker cleanup failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockRejectedValue(setupError);

  const error = await DevEngine.create({}).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(setupError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('dev option getter failure cleans workers before runtime acquisition', async () => {
  const setupError = new Error('dev option getter failed');
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  const devOptions = Object.defineProperty({}, 'onHmrUpdates', {
    get() {
      throw setupError;
    },
  });

  await expect(DevEngine.create({}, {}, devOptions)).rejects.toBe(setupError);
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
});

test('dev snapshots top-level setup options once after worker initialization', async () => {
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const release = vi.fn();
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  let rebuildStrategyReads = 0;
  let watchReads = 0;
  const watch = { pollInterval: 25 };
  const devOptions = {
    get rebuildStrategy() {
      rebuildStrategyReads += 1;
      return 'auto' as const;
    },
    get watch() {
      watchReads += 1;
      return watch;
    },
  };

  await expect(DevEngine.create({}, {}, devOptions)).resolves.toBeInstanceOf(DevEngine);
  expect(rebuildStrategyReads).toBe(1);
  expect(watchReads).toBe(1);
});

test('dev rejects invalid rebuild strategies and cleans initialized workers', async () => {
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));

  await expect(DevEngine.create({}, {}, { rebuildStrategy: 'sometimes' as never })).rejects.toThrow(
    'Invalid dev rebuildStrategy "sometimes". Expected "always", "auto", or "never".',
  );
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
});

test('dev reports non-JSON invalid rebuild strategies without losing cleanup', async () => {
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));

  await expect(DevEngine.create({}, {}, { rebuildStrategy: 1n as never })).rejects.toThrow(
    'Invalid dev rebuildStrategy 1n. Expected "always", "auto", or "never".',
  );
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
});

test('dev construction retries worker cleanup and runtime release', async () => {
  const constructionError = new Error('dev engine construction failed');
  const workerCleanupError = new Error('worker cleanup failed');
  const releaseError = new Error('runtime release failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(workerCleanupError)
    .mockResolvedValue(undefined);
  const release = vi.fn().mockImplementationOnce(() => {
    throw releaseError;
  });
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingConstructionError = constructionError;

  const error = await DevEngine.create({}).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(constructionError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(release).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('dev setup keeps persistent cleanup retryable without hiding the setup error', async () => {
  const constructionError = new Error('dev engine construction failed');
  const firstWorkerCleanupError = new Error('first worker cleanup failed');
  const secondWorkerCleanupError = new Error('second worker cleanup failed');
  const firstReleaseError = new Error('first runtime release failed');
  const secondReleaseError = new Error('second runtime release failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(firstWorkerCleanupError)
    .mockRejectedValueOnce(secondWorkerCleanupError)
    .mockResolvedValue(undefined);
  const release = vi
    .fn()
    .mockImplementationOnce(() => {
      throw firstReleaseError;
    })
    .mockImplementationOnce(() => {
      throw secondReleaseError;
    });
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingConstructionError = constructionError;

  const error = await DevEngine.create({}).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(constructionError);
  expect((error as AggregateError).cause).toBe(constructionError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(release).toHaveBeenCalledTimes(2);
  const cleanup = getRetryableCleanup(error);
  expect(cleanup).toBeDefined();

  await recoverRetryableCleanups();
  expect(stopWorkers).toHaveBeenCalledTimes(3);
  expect(release).toHaveBeenCalledTimes(3);
  expect(hasRetryableCleanupOwnership(cleanup!)).toBe(false);
});

test('dev close retries a transport rejection without releasing owned resources', async () => {
  const transportError = new Error('dev close transport rejected');
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const release = vi.fn();
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingCloseTerminal.mockRejectedValueOnce(transportError).mockResolvedValue(undefined);
  const engine = await DevEngine.create({});

  await expect(engine.close()).rejects.toBe(transportError);
  expect(mocks.bindingCloseTerminal).toHaveBeenCalledOnce();
  expect(stopWorkers).not.toHaveBeenCalled();
  expect(release).not.toHaveBeenCalled();

  await expect(engine.close()).resolves.toBeUndefined();
  expect(mocks.bindingCloseTerminal).toHaveBeenCalledTimes(2);
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

test('dev close memoizes resolved terminal diagnostics after releasing owned resources', async () => {
  const terminalError = new Error('dev close terminal diagnostic');
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const release = vi.fn();
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingCloseTerminal.mockResolvedValue({
    errors: [{ type: 'JsError', field0: terminalError }],
    isBindingErrors: true,
  });
  const engine = await DevEngine.create({});

  await expect(engine.close()).rejects.toBe(terminalError);
  await expect(engine.close()).rejects.toBe(terminalError);
  expect(mocks.bindingCloseTerminal).toHaveBeenCalledOnce();
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

function createBundlerOption(stopWorkers: () => Promise<void>) {
  return {
    bundlerOptions: {},
    inputOptions: {},
    onLog: vi.fn(),
    stopWorkers,
  };
}
