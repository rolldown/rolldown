import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';

export type WorkerData = {
  registryId: number;
  pluginInfos: ParallelPluginInfo[];
  threadNumber: number;
};

type ParallelPluginInfo = {
  index: number;
  fileUrl: string;
  options: unknown;
};

export interface TerminableWorker {
  terminate(): Promise<number>;
}

export interface BootstrapWorker extends TerminableWorker {
  off(event: 'error', listener: (error: Error) => void): this;
  off(event: 'exit', listener: (code: number) => void): this;
  off(event: 'message', listener: (message: unknown) => void): this;
  once(event: 'error', listener: (error: Error) => void): this;
  once(event: 'exit', listener: (code: number) => void): this;
  once(event: 'message', listener: (message: unknown) => void): this;
}

type RetryableCleanup = () => Promise<void>;

const retryableCleanups = new WeakMap<object, RetryableCleanup>();
const cleanupOwnershipChecks = new WeakMap<RetryableCleanup, () => boolean>();
const cleanupFailureErrors = new WeakSet<object>();
const pendingCleanups = new Set<RetryableCleanup>();
const cleanupAttempts = new WeakMap<RetryableCleanup, Promise<void>>();
let pendingCleanupRecovery: Promise<void> | undefined;

/** @internal Associate cleanup ownership with an error without changing its public shape. */
export function attachRetryableCleanup(error: Error, cleanup: RetryableCleanup): void {
  retryableCleanups.set(error, cleanup);
  pendingCleanups.add(cleanup);
}

/** @internal Retrieve cleanup ownership retained by a setup error. */
export function getRetryableCleanup(error: unknown): RetryableCleanup | undefined {
  return typeof error === 'object' && error !== null ? retryableCleanups.get(error) : undefined;
}

/** @internal Tell retry propagation whether a cleanup closure still owns resources. */
export function trackRetryableCleanupOwnership(
  cleanup: RetryableCleanup,
  hasOwnership: () => boolean,
): void {
  cleanupOwnershipChecks.set(cleanup, hasOwnership);
}

/** @internal Identify an aggregate created while associating primary and cleanup failures. */
export function isCleanupFailureError(error: unknown): boolean {
  return typeof error === 'object' && error !== null && cleanupFailureErrors.has(error);
}

function runCleanup(cleanup: RetryableCleanup): Promise<void> {
  const activeAttempt = cleanupAttempts.get(cleanup);
  if (activeAttempt) return activeAttempt;

  const attempt = cleanup().finally(() => {
    if (cleanupAttempts.get(cleanup) === attempt) {
      cleanupAttempts.delete(cleanup);
    }
  });
  cleanupAttempts.set(cleanup, attempt);
  return attempt;
}

/** @internal Recover setup cleanups whose caller discarded the associated error. */
export function recoverRetryableCleanups(): Promise<void> {
  return (pendingCleanupRecovery ??= (async () => {
    const errors: unknown[] = [];
    const cleanups = Array.from(pendingCleanups);
    for (const cleanup of cleanups) {
      pendingCleanups.delete(cleanup);
      try {
        await runCleanup(cleanup);
      } catch (error) {
        if (cleanupOwnershipChecks.get(cleanup)?.() === false) {
          continue;
        }
        pendingCleanups.add(cleanup);
        errors.push(error);
      }
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, 'Pending parallel-plugin worker cleanup failed');
    }
  })().finally(() => {
    pendingCleanupRecovery = undefined;
  }));
}

/** @internal Retry cleanup retained by an earlier setup failure, then rethrow that failure. */
export async function retryCleanupFromError(error: unknown, message: string): Promise<never> {
  const cleanup = getRetryableCleanup(error);
  if (!cleanup) throw error;

  pendingCleanups.delete(cleanup);
  try {
    await runCleanup(cleanup);
  } catch (cleanupError) {
    const hasOwnership = cleanupOwnershipChecks.get(cleanup)?.();
    if (hasOwnership === false) {
      if (typeof error === 'object' && error !== null) {
        retryableCleanups.delete(error);
      }
      throw error;
    }
    pendingCleanups.add(cleanup);
    throw createCleanupFailureError(error, cleanupError, cleanup, message);
  }
  if (typeof error === 'object' && error !== null) {
    retryableCleanups.delete(error);
  }
  throw error;
}

/** @internal Run owned cleanup while preserving both the primary failure and retry ownership. */
export async function cleanupAfterError(
  error: unknown,
  cleanup: RetryableCleanup | undefined,
  message: string,
): Promise<never> {
  if (!cleanup) throw error;
  try {
    await runCleanup(cleanup);
  } catch (cleanupError) {
    throw createCleanupFailureError(error, cleanupError, cleanup, message);
  }
  throw error;
}

/** @internal Keep the primary error first while retaining only live cleanup ownership. */
export function createCleanupFailureError(
  error: unknown,
  cleanupError: unknown,
  cleanup: RetryableCleanup | undefined,
  message: string,
): AggregateError {
  const aggregate = new AggregateError([error, cleanupError], message, { cause: error });
  cleanupFailureErrors.add(aggregate);
  if (cleanup) {
    if (
      typeof cleanupError === 'object' &&
      cleanupError !== null &&
      retryableCleanups.get(cleanupError) === cleanup
    ) {
      retryableCleanups.delete(cleanupError);
    }
    attachRetryableCleanup(aggregate, cleanup);
  }
  return aggregate;
}

/** @internal Retry only workers whose previous termination attempt failed. */
export async function terminateWorkersWithRetry<T extends TerminableWorker>(
  workers: T[],
  maxAttempts: number,
): Promise<{ errors: unknown[]; remainingWorkers: T[] }> {
  let remainingWorkers = workers;
  let errors: unknown[] = [];
  for (let attempt = 0; attempt < maxAttempts && remainingWorkers.length > 0; attempt += 1) {
    const currentWorkers = remainingWorkers;
    const results = await Promise.allSettled(currentWorkers.map((worker) => worker.terminate()));
    remainingWorkers = currentWorkers.filter((_, index) => results[index].status === 'rejected');
    errors = results.flatMap((result) => (result.status === 'rejected' ? [result.reason] : []));
  }
  return { errors, remainingWorkers };
}

export async function initializeParallelPlugins(plugins: RolldownPlugin[]): Promise<
  | {
      registry: ParallelJsPluginRegistry;
      stopWorkers: () => Promise<void>;
    }
  | undefined
> {
  await recoverRetryableCleanups();

  const pluginInfos: ParallelPluginInfo[] = [];
  for (const [index, plugin] of plugins.entries()) {
    if ('_parallel' in plugin) {
      const { fileUrl, options } = plugin._parallel;
      pluginInfos.push({ index, fileUrl, options });
    }
  }
  if (pluginInfos.length <= 0) {
    return undefined;
  }

  const count = availableParallelism();
  const parallelJsPluginRegistry = new ParallelJsPluginRegistry(count);
  const registryId = parallelJsPluginRegistry.id;

  const stopWorkers = await initializeWorkerPool(count, async (threadNumber, registerWorker) => {
    await initializeWorker(registryId, pluginInfos, threadNumber, registerWorker);
  });

  return { registry: parallelJsPluginRegistry, stopWorkers };
}

/** @internal Initialize a pool while retaining every worker from construction onward. */
export async function initializeWorkerPool<T extends TerminableWorker>(
  count: number,
  initializeWorker: (threadNumber: number, registerWorker: (worker: T) => void) => Promise<void>,
): Promise<RetryableCleanup> {
  const workers: T[] = [];
  const registeredWorkers = new Set<T>();
  const registerWorker = (worker: T) => {
    if (!registeredWorkers.has(worker)) {
      registeredWorkers.add(worker);
      workers.push(worker);
    }
  };
  const stopWorkers = createWorkerCleanup(workers);

  const results = await Promise.allSettled(
    Array.from({ length: count }, (_, threadNumber) =>
      initializeWorker(threadNumber, registerWorker),
    ),
  );
  const errors = results.flatMap((result) => (result.status === 'rejected' ? [result.reason] : []));
  if (errors.length > 0) {
    const error =
      errors.length === 1
        ? errors[0]
        : new AggregateError(errors, 'Multiple parallel-plugin workers failed to initialize');
    await cleanupAfterError(
      error,
      stopWorkers,
      'Parallel-plugin worker initialization and cleanup both failed',
    );
  }
  return stopWorkers;
}

function createWorkerCleanup<T extends TerminableWorker>(initialWorkers: T[]): RetryableCleanup {
  let workers = initialWorkers;
  const stopWorkers: RetryableCleanup = async () => {
    const result = await terminateWorkersWithRetry(workers, 1);
    workers = result.remainingWorkers;
    if (result.errors.length === 0) {
      pendingCleanups.delete(stopWorkers);
      return;
    }
    const error =
      result.errors.length === 1
        ? result.errors[0]
        : new AggregateError(result.errors, 'Parallel-plugin worker shutdown failed');
    const retryableError =
      error instanceof Error
        ? error
        : new AggregateError([error], 'Parallel-plugin worker shutdown failed');
    throw retryableError;
  };
  trackRetryableCleanupOwnership(stopWorkers, () => workers.length > 0);
  return stopWorkers;
}

async function initializeWorker(
  registryId: number,
  pluginInfos: ParallelPluginInfo[],
  threadNumber: number,
  registerWorker: (worker: Worker) => void,
) {
  const urlString = import.meta.resolve('#parallel-plugin-worker');
  const workerData: WorkerData = {
    registryId,
    pluginInfos,
    threadNumber,
  };

  const worker = new Worker(new URL(urlString), { workerData });
  registerWorker(worker);
  worker.unref();
  await waitForWorkerBootstrap(worker);
}

/** @internal Wait for the worker bootstrap protocol and reject on transport failure. */
export function waitForWorkerBootstrap(worker: BootstrapWorker): Promise<void> {
  return new Promise((resolve, reject) => {
    const cleanupListeners = () => {
      worker.off('message', onMessage);
      worker.off('error', onError);
      worker.off('exit', onExit);
    };
    const settle = (callback: () => void) => {
      cleanupListeners();
      callback();
    };
    const onMessage = (message: unknown) => {
      if (
        typeof message === 'object' &&
        message !== null &&
        'type' in message &&
        message.type === 'success'
      ) {
        settle(resolve);
        return;
      }
      if (
        typeof message === 'object' &&
        message !== null &&
        'type' in message &&
        message.type === 'error'
      ) {
        settle(() => reject('error' in message ? message.error : message));
        return;
      }
      settle(() => reject(new Error('Parallel-plugin worker sent an invalid bootstrap response')));
    };
    const onError = (error: Error) => {
      settle(() => reject(error));
    };
    const onExit = (code: number) => {
      settle(() =>
        reject(
          new Error(
            `Parallel-plugin worker exited before initialization completed (exit code ${code})`,
          ),
        ),
      );
    };

    worker.once('message', onMessage);
    worker.once('error', onError);
    worker.once('exit', onExit);
  });
}

const availableParallelism = () => {
  let availableParallelism = 1;
  try {
    availableParallelism = os.availableParallelism();
  } catch {
    const cpus = os.cpus();
    if (Array.isArray(cpus) && cpus.length > 0) {
      availableParallelism = cpus.length;
    }
  }
  return Math.min(availableParallelism, 8);
};
