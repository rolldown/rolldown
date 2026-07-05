// @ts-nocheck This focused unit test intentionally reaches package source outside the test rootDir.
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  bindingifyInputOptions: vi.fn(),
  initializeParallelPlugins: vi.fn(),
}));

vi.mock('../src/utils/bindingify-input-options', () => ({
  bindingifyInputOptions: mocks.bindingifyInputOptions,
}));

vi.mock('../src/utils/initialize-parallel-plugins', async (importOriginal) => ({
  ...(await importOriginal()),
  initializeParallelPlugins: mocks.initializeParallelPlugins,
}));

import { createBundlerOptions } from '../src/utils/create-bundler-option';
import { getRetryableCleanup } from '../src/utils/retryable-cleanup';

beforeEach(() => {
  mocks.bindingifyInputOptions.mockReset();
  mocks.initializeParallelPlugins.mockReset();
});

test('bundler-option setup retries the retained worker after termination first rejects', async () => {
  const setupError = new Error('binding option conversion failed');
  const cleanupError = new Error('worker termination failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.initializeParallelPlugins.mockResolvedValue({
    registry: {},
    stopWorkers,
  });
  mocks.bindingifyInputOptions.mockImplementation(() => {
    throw setupError;
  });

  const error = await createBundlerOptions({}, {}, false).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([setupError, cleanupError]);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});
