// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { terminateWorkersWithRetry } from '../src/utils/initialize-parallel-plugins';
import { describe, expect, test, vi } from 'vitest';

describe('parallel plugin worker cleanup', () => {
  test('retries only workers whose first termination attempt failed', async () => {
    const recovered = {
      terminate: vi
        .fn<() => Promise<number>>()
        .mockRejectedValueOnce(new Error('first termination failed'))
        .mockResolvedValue(0),
    };
    const completed = {
      terminate: vi.fn<() => Promise<number>>().mockResolvedValue(0),
    };

    const result = await terminateWorkersWithRetry([recovered, completed], 2);

    expect(result).toEqual({ errors: [], remainingWorkers: [] });
    expect(recovered.terminate).toHaveBeenCalledTimes(2);
    expect(completed.terminate).toHaveBeenCalledOnce();
  });

  test('retains workers that still fail after the bounded retry', async () => {
    const error = new Error('termination failed');
    const worker = {
      terminate: vi.fn<() => Promise<number>>().mockRejectedValue(error),
    };

    const result = await terminateWorkersWithRetry([worker], 2);

    expect(result.errors).toEqual([error]);
    expect(result.remainingWorkers).toEqual([worker]);
    expect(worker.terminate).toHaveBeenCalledTimes(2);
  });
});
