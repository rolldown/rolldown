// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingConstructionError: undefined as unknown,
  createBundlerOptions: vi.fn(),
}));

vi.mock('../src/binding.cjs', () => ({
  BindingWatcher: class {
    constructor() {
      if (mocks.bindingConstructionError) throw mocks.bindingConstructionError;
    }
  },
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: vi.fn(async (option) => option),
  },
}));

vi.mock('../src/runtime-lifecycle', () => ({
  acquireRuntimeLease: mocks.acquireRuntimeLease,
  CloseCoordinator: class {},
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: mocks.createBundlerOptions,
}));

import { createWatcher } from '../src/api/watch/watcher';
import {
  createCleanupFailureError,
  getRetryableCleanup,
  hasRetryableCleanupOwnership,
  recoverRetryableCleanups,
} from '../src/utils/retryable-cleanup';

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.bindingConstructionError = undefined;
  mocks.createBundlerOptions.mockReset();
});

test('partial watcher option setup retries cleanup from fulfilled and rejected options', async () => {
  const optionSetupError = new Error('option setup failed');
  const priorCleanupError = new Error('failed option cleanup failed');
  const fulfilledCleanupError = new Error('fulfilled option cleanup failed');
  const fulfilledStopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(fulfilledCleanupError)
    .mockResolvedValue(undefined);
  const rejectedStopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(priorCleanupError)
    .mockResolvedValue(undefined);
  const rejectedOptionError = createCleanupFailureError(
    optionSetupError,
    priorCleanupError,
    rejectedStopWorkers,
    'Option setup and cleanup failed',
  );
  mocks.createBundlerOptions
    .mockResolvedValueOnce(createBundlerOption(fulfilledStopWorkers))
    .mockRejectedValueOnce(rejectedOptionError);

  const error = await createWatcher({}, [{ output: {} }, { output: {} }]).catch(
    (error: unknown) => error,
  );

  expect(error).toBeInstanceOf(AggregateError);
  expect(fulfilledStopWorkers).toHaveBeenCalledTimes(2);
  expect(rejectedStopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('watcher runtime setup retries failed worker cleanup', async () => {
  const setupError = new Error('runtime lease setup failed');
  const cleanupError = new Error('worker cleanup failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockImplementation(() => {
    throw setupError;
  });

  const error = await createWatcher({}, { output: {} }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(setupError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('watcher construction retries worker cleanup and runtime release', async () => {
  const constructionError = new Error('watcher construction failed');
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
  mocks.acquireRuntimeLease.mockReturnValue({ release });
  mocks.bindingConstructionError = constructionError;

  const error = await createWatcher({}, { output: {} }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(constructionError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(release).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('watcher setup keeps persistent cleanup retryable without hiding the setup error', async () => {
  const setupError = new Error('runtime lease setup failed');
  const firstCleanupError = new Error('first worker cleanup failed');
  const secondCleanupError = new Error('second worker cleanup failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(firstCleanupError)
    .mockRejectedValueOnce(secondCleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockImplementation(() => {
    throw setupError;
  });

  const error = await createWatcher({}, { output: {} }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([
    setupError,
    firstCleanupError,
    secondCleanupError,
  ]);
  expect((error as AggregateError).cause).toBe(setupError);
  const cleanup = getRetryableCleanup(error);
  expect(cleanup).toBeDefined();
  expect(stopWorkers).toHaveBeenCalledTimes(2);

  await recoverRetryableCleanups();
  expect(stopWorkers).toHaveBeenCalledTimes(3);
  expect(hasRetryableCleanupOwnership(cleanup!)).toBe(false);
});

function createBundlerOption(stopWorkers: () => Promise<void>) {
  return {
    bundlerOptions: {},
    inputOptions: {},
    onLog: vi.fn(),
    stopWorkers,
  };
}
