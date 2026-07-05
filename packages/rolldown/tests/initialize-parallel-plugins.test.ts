// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { EventEmitter } from 'node:events';
import {
  getRetryableCleanup,
  initializeWorkerPool,
  recoverRetryableCleanups,
  retryCleanupFromError,
  terminateWorkersWithRetry,
  waitForWorkerBootstrap,
} from '../src/utils/initialize-parallel-plugins';
import { describe, expect, test, vi } from 'vitest';

class TestWorker extends EventEmitter {
  terminate = vi.fn<() => Promise<number>>().mockResolvedValue(0);
}

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

  test('does not mistake a falsey termination rejection for success', async () => {
    const worker = {
      terminate: vi.fn<() => Promise<number>>().mockRejectedValue(undefined),
    };

    const result = await terminateWorkersWithRetry([worker], 1);

    expect(result.errors).toEqual([undefined]);
    expect(result.remainingWorkers).toEqual([worker]);
  });

  test('retains cleanup ownership when worker shutdown rejects with a falsey value', async () => {
    const startupError = new Error('worker bootstrap failed');
    const worker = new TestWorker();
    worker.terminate.mockRejectedValueOnce(undefined).mockResolvedValue(0);

    const result = initializeWorkerPool<TestWorker>(1, async (_, registerWorker) => {
      registerWorker(worker);
      throw startupError;
    });

    const setupError = await result.catch((error: unknown) => error);
    expect(setupError).toBeInstanceOf(AggregateError);
    const cleanupError = (setupError as AggregateError).errors[1];
    expect(cleanupError).toBeInstanceOf(AggregateError);
    expect((cleanupError as AggregateError).errors).toEqual([undefined]);

    await getRetryableCleanup(setupError)?.();
    expect(worker.terminate).toHaveBeenCalledTimes(2);
  });

  test('recovers abandoned setup cleanup ownership on the next initialization', async () => {
    const worker = new TestWorker();
    worker.terminate.mockRejectedValueOnce(new Error('first termination failed'));
    worker.terminate.mockResolvedValue(0);

    await initializeWorkerPool<TestWorker>(1, async (_, registerWorker) => {
      registerWorker(worker);
      throw new Error('worker bootstrap failed');
    }).catch(() => {});

    await recoverRetryableCleanups();
    expect(worker.terminate).toHaveBeenCalledTimes(2);
  });

  test('coalesces abandoned recovery with an explicit cleanup retry', async () => {
    let finishTermination!: (exitCode: number) => void;
    const worker = new TestWorker();
    worker.terminate.mockRejectedValueOnce(new Error('first termination failed'));
    worker.terminate.mockImplementationOnce(
      () =>
        new Promise<number>((resolve) => {
          finishTermination = resolve;
        }),
    );

    const setupError = await initializeWorkerPool<TestWorker>(1, async (_, registerWorker) => {
      registerWorker(worker);
      throw new Error('worker bootstrap failed');
    }).catch((error: unknown) => error);

    const recovery = recoverRetryableCleanups();
    const retry = retryCleanupFromError(setupError, 'retry failed').catch(
      (error: unknown) => error,
    );
    expect(worker.terminate).toHaveBeenCalledTimes(2);
    finishTermination(0);

    await expect(recovery).resolves.toBeUndefined();
    await expect(retry).resolves.toBe(setupError);
    expect(worker.terminate).toHaveBeenCalledTimes(2);
  });

  test('rejects bootstrap on a worker transport error', async () => {
    const worker = new TestWorker();
    const startupError = new Error('worker failed before bootstrap');

    const result = waitForWorkerBootstrap(worker);
    worker.emit('error', startupError);

    await expect(result).rejects.toBe(startupError);
    expect(worker.listenerCount('message')).toBe(0);
    expect(worker.listenerCount('error')).toBe(0);
    expect(worker.listenerCount('exit')).toBe(0);
  });

  test('rejects bootstrap when a worker exits before reporting readiness', async () => {
    const worker = new TestWorker();

    const result = waitForWorkerBootstrap(worker);
    worker.emit('exit', 17);

    await expect(result).rejects.toThrow(
      'Parallel-plugin worker exited before initialization completed (exit code 17)',
    );
    expect(worker.listenerCount('message')).toBe(0);
    expect(worker.listenerCount('error')).toBe(0);
    expect(worker.listenerCount('exit')).toBe(0);
  });

  test('cleans partially started siblings and retains a failed termination for retry', async () => {
    const startupError = new Error('worker bootstrap failed');
    const terminationError = new Error('first termination failed');
    const workers = Array.from({ length: 3 }, () => new TestWorker());
    workers[0].terminate.mockRejectedValueOnce(terminationError).mockResolvedValue(0);

    const result = initializeWorkerPool<TestWorker>(
      workers.length,
      async (threadNumber, registerWorker) => {
        registerWorker(workers[threadNumber]);
        if (threadNumber === 1) {
          throw startupError;
        }
      },
    );

    const setupError = await result.catch((error: unknown) => error);
    expect(setupError).toBeInstanceOf(AggregateError);
    expect((setupError as AggregateError).errors).toEqual([startupError, terminationError]);
    for (const worker of workers) {
      expect(worker.terminate).toHaveBeenCalledOnce();
    }

    const retryCleanup = getRetryableCleanup(setupError);
    expect(retryCleanup).toBeDefined();
    await retryCleanup?.();
    expect(workers[0].terminate).toHaveBeenCalledTimes(2);
    expect(workers[1].terminate).toHaveBeenCalledOnce();
    expect(workers[2].terminate).toHaveBeenCalledOnce();
  });

  test('waits for late worker registration before taking the cleanup snapshot', async () => {
    let finishSiblingBootstrap!: () => void;
    const startupError = new Error('first worker bootstrap failed');
    const workers = [new TestWorker(), new TestWorker()];

    const result = initializeWorkerPool<TestWorker>(
      workers.length,
      async (threadNumber, registerWorker) => {
        if (threadNumber === 0) {
          registerWorker(workers[0]);
          throw startupError;
        }
        await new Promise<void>((resolve) => {
          finishSiblingBootstrap = resolve;
        });
        registerWorker(workers[1]);
      },
    );

    await Promise.resolve();
    expect(workers[0].terminate).not.toHaveBeenCalled();
    finishSiblingBootstrap();

    await expect(result).rejects.toBe(startupError);
    expect(workers[0].terminate).toHaveBeenCalledOnce();
    expect(workers[1].terminate).toHaveBeenCalledOnce();
  });
});
