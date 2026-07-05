// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingConstructionError: undefined as unknown,
  createBundlerOptions: vi.fn(),
}));

vi.mock('../src/binding.cjs', () => ({
  BindingDevEngine: class {
    constructor() {
      if (mocks.bindingConstructionError) throw mocks.bindingConstructionError;
    }
  },
  BindingRebuildStrategy: {
    Always: 'always',
    Auto: 'auto',
    Never: 'never',
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

import { DevEngine } from '../src/api/dev/dev-engine';
import {
  getRetryableCleanup,
  hasRetryableCleanupOwnership,
  recoverRetryableCleanups,
} from '../src/utils/retryable-cleanup';

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.bindingConstructionError = undefined;
  mocks.createBundlerOptions.mockReset();
});

test('dev runtime setup retries failed worker cleanup', async () => {
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

  const error = await DevEngine.create({}).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(setupError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
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
  mocks.acquireRuntimeLease.mockReturnValue({ release });
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
  mocks.acquireRuntimeLease.mockReturnValue({ release });
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

function createBundlerOption(stopWorkers: () => Promise<void>) {
  return {
    bundlerOptions: {},
    inputOptions: {},
    onLog: vi.fn(),
    stopWorkers,
  };
}
