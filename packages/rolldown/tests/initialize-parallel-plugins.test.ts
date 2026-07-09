// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { EventEmitter } from 'node:events';
import {
  createParallelPluginWorkerEnv,
  initializeWorkerPool,
  sanitizeFileWorkerExecArgv,
  superviseWorker,
  type SupervisedWorker,
  terminateWorkersWithRetry,
  WorkerBootstrapCoordinator,
} from '../src/utils/initialize-parallel-plugins';
import {
  getRetryableCleanup,
  recoverRetryableCleanups,
  retryCleanupFromError,
} from '../src/utils/retryable-cleanup';
import { describe, expect, test, vi } from 'vitest';

class TestWorker extends EventEmitter {
  postMessage = vi.fn<(message: unknown) => void>();
  terminate = vi.fn<() => Promise<number>>().mockResolvedValue(0);
}

const testAuthentication = {
  readyToken: 'test-ready-token',
  resultToken: 'test-result-token',
  session: 'test-session',
  startToken: 'test-start-token',
};

function superviseTestWorker(worker: TestWorker): SupervisedWorker {
  return superviseWorker(worker, testAuthentication);
}

function authenticatedBootstrapMessage(
  type: 'ready' | 'success' | 'error',
  error?: unknown,
): Record<string, unknown> {
  return {
    ...(type === 'error' ? { error } : {}),
    session: testAuthentication.session,
    token: type === 'ready' ? testAuthentication.readyToken : testAuthentication.resultToken,
    type,
  };
}

function reportReady(worker: TestWorker): void {
  worker.emit('message', authenticatedBootstrapMessage('ready'));
}

function completeBootstrap(worker: TestWorker, supervisedWorker: SupervisedWorker): void {
  reportReady(worker);
  supervisedWorker.startBootstrap();
  worker.emit('message', authenticatedBootstrapMessage('success'));
}

describe('parallel plugin worker cleanup', () => {
  test('sanitizes parent invocation modes and inherited code injection flags', () => {
    expect(
      sanitizeFileWorkerExecArgv([
        '--input-type=module',
        '--eval',
        'import("./child.mjs")',
        '-p',
        'process.version',
        '--check',
        '--interactive',
        '--run',
        'parent-script',
        '--import',
        './register.mjs',
        '--require=./register.cjs',
        '-r',
        './register-short.cjs',
        '--experimental-loader=./loader.mjs',
        '--loader',
        './legacy-loader.mjs',
        '--conditions',
        'development',
        '--trace-warnings',
      ]),
    ).toEqual(['--conditions', 'development', '--trace-warnings']);

    expect(
      sanitizeFileWorkerExecArgv([
        '--input-type',
        'commonjs',
        '--eval=0',
        '--print=process.version',
        '--run=parent-script',
        '-e',
        '0',
        '-c',
        '-i',
      ]),
    ).toEqual([]);
  });

  test('clears NODE_OPTIONS without mutating the parent environment', () => {
    const source = {
      NODE_OPTIONS: '--import ./preload.mjs',
      PATH: '/test/bin',
    };

    expect(createParallelPluginWorkerEnv(source)).toEqual({
      NODE_OPTIONS: '',
      PATH: '/test/bin',
    });
    expect(source.NODE_OPTIONS).toBe('--import ./preload.mjs');
  });

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
    const supervisedWorker = superviseTestWorker(worker);

    const result = supervisedWorker.waitForBootstrap();
    worker.emit('error', startupError);

    await expect(result).rejects.toBe(startupError);
    expect(worker.listenerCount('message')).toBe(0);
    expect(worker.listenerCount('error')).toBe(1);
    expect(worker.listenerCount('exit')).toBe(1);
    const termination = supervisedWorker.terminate();
    expect(worker.terminate).not.toHaveBeenCalled();
    worker.emit('exit', 1);
    await expect(termination).resolves.toBe(1);
    expect(worker.listenerCount('error')).toBe(0);
    expect(worker.listenerCount('exit')).toBe(0);
  });

  test('rejects bootstrap when a worker exits before reporting readiness', async () => {
    const worker = new TestWorker();
    const supervisedWorker = superviseTestWorker(worker);

    const result = supervisedWorker.waitForBootstrap();
    worker.emit('exit', 17);

    await expect(result).rejects.toThrow(
      'Parallel-plugin worker exited before initialization completed (exit code 17)',
    );
    expect(worker.listenerCount('message')).toBe(0);
    expect(worker.listenerCount('error')).toBe(0);
    expect(worker.listenerCount('exit')).toBe(0);
  });

  test('ignores unauthenticated bootstrap messages', async () => {
    const worker = new TestWorker();
    const supervisedWorker = superviseTestWorker(worker);
    const bootstrap = supervisedWorker.waitForBootstrap();
    let settled = false;
    void bootstrap.then(
      () => {
        settled = true;
      },
      () => {
        settled = true;
      },
    );

    worker.emit('message', { type: 'ready' });
    worker.emit('message', { type: 'success' });
    await new Promise<void>((resolve) => setImmediate(resolve));

    expect(settled).toBe(false);
    expect(worker.postMessage).not.toHaveBeenCalled();

    completeBootstrap(worker, supervisedWorker);
    await expect(bootstrap).resolves.toBeUndefined();
    await expect(supervisedWorker.terminate()).resolves.toBe(0);
  });

  test('rejects an authenticated terminal message before the start phase', async () => {
    const worker = new TestWorker();
    const supervisedWorker = superviseTestWorker(worker);
    const bootstrap = supervisedWorker.waitForBootstrap();

    reportReady(worker);
    worker.emit('message', authenticatedBootstrapMessage('success'));

    await expect(bootstrap).rejects.toThrow(
      'Parallel-plugin worker sent an out-of-order or duplicate terminal message',
    );
    expect(worker.postMessage).not.toHaveBeenCalled();
    await expect(supervisedWorker.terminate()).resolves.toBe(0);
  });

  test('rejects duplicate authenticated readiness and terminal messages', async () => {
    const readinessWorker = new TestWorker();
    const readinessSupervisor = superviseTestWorker(readinessWorker);
    const readinessBootstrap = readinessSupervisor.waitForBootstrap();
    reportReady(readinessWorker);
    reportReady(readinessWorker);
    await expect(readinessBootstrap).rejects.toThrow(
      'Parallel-plugin worker sent an invalid or duplicate ready message',
    );
    await expect(readinessSupervisor.terminate()).resolves.toBe(0);

    const terminalWorker = new TestWorker();
    const terminalSupervisor = superviseTestWorker(terminalWorker);
    completeBootstrap(terminalWorker, terminalSupervisor);
    await terminalSupervisor.waitForBootstrap();
    terminalWorker.emit('message', authenticatedBootstrapMessage('success'));
    await expect(terminalSupervisor.terminate()).rejects.toThrow(
      'Parallel-plugin worker sent an out-of-order or duplicate terminal message',
    );
  });

  test('starts every worker only after the full pool reports readiness', async () => {
    const workers = [new TestWorker(), new TestWorker()];
    const supervisors = workers.map(superviseTestWorker);
    const coordinator = new WorkerBootstrapCoordinator(workers.length);
    supervisors.forEach((worker, threadNumber) => coordinator.register(threadNumber, worker));

    reportReady(workers[0]);
    await supervisors[0].waitForReadiness();
    const firstStart = coordinator.markReady(0);
    await Promise.resolve();
    expect(workers[0].postMessage).not.toHaveBeenCalled();
    expect(workers[1].postMessage).not.toHaveBeenCalled();

    reportReady(workers[1]);
    await supervisors[1].waitForReadiness();
    const secondStart = coordinator.markReady(1);
    await Promise.all([firstStart, secondStart]);

    for (const worker of workers) {
      expect(worker.postMessage).toHaveBeenCalledOnce();
      expect(worker.postMessage).toHaveBeenCalledWith({
        session: testAuthentication.session,
        token: testAuthentication.startToken,
        type: 'start',
      });
      worker.emit('message', authenticatedBootstrapMessage('success'));
    }
    await Promise.all(supervisors.map((worker) => worker.waitForBootstrap()));
    await Promise.all(supervisors.map((worker) => worker.terminate()));
  });

  test('terminating a bootstrapping worker settles its initializer', async () => {
    const worker = new TestWorker();
    const supervisedWorker = superviseTestWorker(worker);
    const bootstrap = supervisedWorker.waitForBootstrap();
    const bootstrapRejection = expect(bootstrap).rejects.toThrow(
      'Parallel-plugin worker initialization was cancelled during pool cleanup',
    );

    const termination = supervisedWorker.terminate();
    await bootstrapRejection;
    expect(worker.terminate).not.toHaveBeenCalled();
    reportReady(worker);
    await expect(termination).resolves.toBe(0);
    expect(worker.terminate).toHaveBeenCalledOnce();
    expect(worker.listenerCount('message')).toBe(0);
    expect(worker.listenerCount('error')).toBe(0);
    expect(worker.listenerCount('exit')).toBe(0);
  });

  test('authenticated readiness received during cleanup releases termination', async () => {
    const worker = new TestWorker();
    const supervisedWorker = superviseTestWorker(worker);
    const bootstrap = supervisedWorker.waitForBootstrap();
    const bootstrapRejection = expect(bootstrap).rejects.toThrow(
      'Parallel-plugin worker initialization was cancelled during pool cleanup',
    );

    const termination = supervisedWorker.terminate();
    await bootstrapRejection;
    expect(worker.terminate).not.toHaveBeenCalled();
    reportReady(worker);
    await expect(termination).resolves.toBe(0);
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  test('retains delayed worker errors until shutdown without reterminating the worker', async () => {
    const worker = new TestWorker();
    const workerError = new Error('worker failed after bootstrap');
    const supervisedWorker = superviseTestWorker(worker);
    completeBootstrap(worker, supervisedWorker);
    await supervisedWorker.waitForBootstrap();

    expect(worker.listenerCount('error')).toBe(1);
    expect(worker.listenerCount('exit')).toBe(1);
    worker.emit('error', workerError);

    await expect(supervisedWorker.terminate()).rejects.toBe(workerError);
    expect(worker.terminate).toHaveBeenCalledOnce();
    expect(worker.listenerCount('error')).toBe(0);
    expect(worker.listenerCount('exit')).toBe(0);
    await expect(supervisedWorker.terminate()).resolves.toBe(0);
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  test('ignores transport errors emitted by intentional worker termination', async () => {
    const worker = new TestWorker();
    const closingError = Object.assign(new Error('The worker environment is closing'), {
      code: 'Closing',
    });
    const supervisedWorker = superviseTestWorker(worker);
    completeBootstrap(worker, supervisedWorker);
    await supervisedWorker.waitForBootstrap();
    worker.terminate.mockImplementationOnce(async () => {
      worker.emit('error', closingError);
      return 0;
    });

    await expect(supervisedWorker.terminate()).resolves.toBe(0);
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  test('retries physical termination after a delayed fault and first termination rejection', async () => {
    const workerFault = new Error('worker failed after bootstrap');
    const terminationError = new Error('first termination failed');
    const worker = new TestWorker();
    worker.terminate.mockRejectedValueOnce(terminationError).mockResolvedValue(0);

    const stopWorkers = await initializeWorkerPool<SupervisedWorker>(
      1,
      async (_, registerWorker) => {
        const supervisedWorker = superviseTestWorker(worker);
        registerWorker(supervisedWorker);
        completeBootstrap(worker, supervisedWorker);
        await supervisedWorker.waitForBootstrap();
      },
    );
    worker.emit('error', workerFault);

    const firstError = await stopWorkers().catch((error: unknown) => error);
    expect(firstError).toBeInstanceOf(AggregateError);
    expect((firstError as AggregateError).errors).toEqual([workerFault, terminationError]);
    expect(worker.terminate).toHaveBeenCalledOnce();

    await expect(stopWorkers()).resolves.toBeUndefined();
    expect(worker.terminate).toHaveBeenCalledTimes(2);
  });

  test('retains unexpected worker exits until cleanup observes them', async () => {
    const worker = new TestWorker();
    const supervisedWorker = superviseTestWorker(worker);
    completeBootstrap(worker, supervisedWorker);
    await supervisedWorker.waitForBootstrap();

    worker.emit('exit', 23);

    await expect(supervisedWorker.terminate()).rejects.toThrow(
      'Parallel-plugin worker exited unexpectedly (exit code 23)',
    );
    expect(worker.terminate).not.toHaveBeenCalled();
    await expect(supervisedWorker.terminate()).resolves.toBe(23);
    expect(worker.terminate).not.toHaveBeenCalled();
  });

  test('keeps setup errors primary when a bootstrapped worker faults before cleanup', async () => {
    const setupError = new Error('pool setup failed');
    const workerError = new Error('worker failed after bootstrap');
    const worker = new TestWorker();

    const result = initializeWorkerPool<SupervisedWorker>(1, async (_, registerWorker) => {
      const supervisedWorker = superviseTestWorker(worker);
      registerWorker(supervisedWorker);
      completeBootstrap(worker, supervisedWorker);
      await supervisedWorker.waitForBootstrap();
      worker.emit('error', workerError);
      throw setupError;
    });

    const error = await result.catch((error: unknown) => error);
    expect(error).toBeInstanceOf(AggregateError);
    expect((error as AggregateError).errors).toEqual([setupError, workerError]);
    expect((error as AggregateError).cause).toBe(setupError);
    expect(worker.terminate).toHaveBeenCalledOnce();

    await expect(getRetryableCleanup(error)?.()).resolves.toBeUndefined();
    expect(worker.terminate).toHaveBeenCalledOnce();
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

  test('cleans registered siblings without waiting for a bootstrap that never settles', async () => {
    const startupError = new Error('first worker bootstrap failed');
    const workers = [new TestWorker(), new TestWorker()];
    const neverSettles = new Promise<void>(() => {});

    const result = initializeWorkerPool<TestWorker>(
      workers.length,
      async (threadNumber, registerWorker) => {
        registerWorker(workers[threadNumber]);
        if (threadNumber === 0) throw startupError;
        await neverSettles;
      },
    );
    const outcome = await Promise.race([
      result.then(
        () => ({ status: 'resolved' as const }),
        (error: unknown) => ({ error, status: 'rejected' as const }),
      ),
      new Promise<{ status: 'pending' }>((resolve) => {
        setImmediate(() => resolve({ status: 'pending' }));
      }),
    ]);

    expect(outcome).toEqual({ error: startupError, status: 'rejected' });
    expect(workers[0].terminate).toHaveBeenCalledOnce();
    expect(workers[1].terminate).toHaveBeenCalledOnce();
  });
});
