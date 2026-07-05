import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  close: vi.fn(),
  createBundlerOptions: vi.fn(),
  scan: vi.fn(),
}));

vi.mock('../src/binding.cjs', () => ({
  BindingBundler: class {
    close = mocks.close;
    scan = mocks.scan;
  },
}));

vi.mock('../src/runtime-lifecycle', () => ({
  acquireRuntimeLease: mocks.acquireRuntimeLease,
}));

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: mocks.createBundlerOptions,
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { scan } from '../src/api/experimental';
// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { getRetryableCleanup } from '../src/utils/initialize-parallel-plugins';

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.close.mockReset();
  mocks.createBundlerOptions.mockReset();
  mocks.scan.mockReset();
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
  mocks.acquireRuntimeLease.mockImplementation(() => {
    throw setupError;
  });

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
  mocks.acquireRuntimeLease.mockReturnValue({ release });
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
