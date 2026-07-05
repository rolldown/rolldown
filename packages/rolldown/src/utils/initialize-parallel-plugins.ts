import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';
import {
  cleanupAfterError,
  clearRetryableCleanup,
  recoverRetryableCleanups,
  trackRetryableCleanupOwnership,
  type RetryableCleanup,
} from './retryable-cleanup';

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
  on(event: 'error', listener: (error: Error) => void): this;
  on(event: 'exit', listener: (code: number) => void): this;
  off(event: 'error', listener: (error: Error) => void): this;
  off(event: 'exit', listener: (code: number) => void): this;
  off(event: 'message', listener: (message: unknown) => void): this;
  once(event: 'message', listener: (message: unknown) => void): this;
}

export interface SupervisedWorker extends TerminableWorker {
  waitForBootstrap(): Promise<void>;
}

const FILE_WORKER_CONTEXT_FLAGS_WITH_VALUE = new Set([
  '--eval',
  '-e',
  '--input-type',
  '--print',
  '-p',
  '--run',
]);
const FILE_WORKER_CONTEXT_FLAGS = new Set(['--check', '-c', '--interactive', '-i']);

/** @internal Remove parent invocation modes that are invalid or meaningless for a file worker. */
export function sanitizeFileWorkerExecArgv(execArgv: readonly string[]): string[] {
  const sanitized: string[] = [];
  for (let index = 0; index < execArgv.length; index += 1) {
    const argument = execArgv[index];
    const equalsIndex = argument.indexOf('=');
    const flag = equalsIndex === -1 ? argument : argument.slice(0, equalsIndex);
    if (FILE_WORKER_CONTEXT_FLAGS_WITH_VALUE.has(flag)) {
      if (equalsIndex === -1) {
        index += 1;
      }
      continue;
    }
    if (FILE_WORKER_CONTEXT_FLAGS.has(argument)) {
      continue;
    }
    sanitized.push(argument);
  }
  return sanitized;
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

  const stopWorkers = await initializeWorkerPool<SupervisedWorker>(
    count,
    async (threadNumber, registerWorker) => {
      await initializeWorker(registryId, pluginInfos, threadNumber, registerWorker);
    },
  );

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
      clearRetryableCleanup(stopWorkers);
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
  registerWorker: (worker: SupervisedWorker) => void,
) {
  const urlString = import.meta.resolve('#parallel-plugin-worker');
  const workerData: WorkerData = {
    registryId,
    pluginInfos,
    threadNumber,
  };

  const worker = new Worker(new URL(urlString), {
    workerData,
    execArgv: sanitizeFileWorkerExecArgv(process.execArgv),
  });
  const supervisedWorker = superviseWorker(worker);
  registerWorker(supervisedWorker);
  try {
    await supervisedWorker.waitForBootstrap();
  } finally {
    worker.unref();
  }
}

/** @internal Retain worker fault supervision from construction through shutdown. */
export function superviseWorker(worker: BootstrapWorker): SupervisedWorker {
  return new WorkerSupervisor(worker);
}

type WorkerPhase = 'bootstrapping' | 'running' | 'failed' | 'stopping' | 'stopped';

class WorkerSupervisor implements SupervisedWorker {
  readonly #worker: BootstrapWorker;
  readonly #bootstrapPromise: Promise<void>;
  #resolveBootstrap!: () => void;
  #rejectBootstrap!: (error: unknown) => void;
  #phase: WorkerPhase = 'bootstrapping';
  #faults: unknown[] = [];
  #exitCode = 0;

  constructor(worker: BootstrapWorker) {
    this.#worker = worker;
    this.#bootstrapPromise = new Promise<void>((resolve, reject) => {
      this.#resolveBootstrap = resolve;
      this.#rejectBootstrap = reject;
    });
    worker.once('message', this.#onMessage);
    worker.on('error', this.#onError);
    worker.on('exit', this.#onExit);
  }

  waitForBootstrap(): Promise<void> {
    return this.#bootstrapPromise;
  }

  async terminate(): Promise<number> {
    let terminationError: unknown;
    let hasTerminationError = false;
    const previousPhase = this.#phase;
    if (this.#phase !== 'stopped') {
      this.#phase = 'stopping';
      try {
        this.#exitCode = await this.#worker.terminate();
        this.#phase = 'stopped';
        this.#disposeListeners();
      } catch (error) {
        terminationError = error;
        hasTerminationError = true;
        if (this.#phase !== 'stopped') {
          this.#phase = previousPhase === 'running' ? 'running' : 'failed';
        }
      }
    }

    const errors = this.#faults;
    this.#faults = [];
    if (hasTerminationError) {
      errors.push(terminationError);
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, 'Parallel-plugin worker fault or shutdown failed', {
        cause: errors[0],
      });
    }
    return this.#exitCode;
  }

  readonly #onMessage = (message: unknown) => {
    if (this.#phase !== 'bootstrapping') return;
    this.#worker.off('message', this.#onMessage);
    if (
      typeof message === 'object' &&
      message !== null &&
      'type' in message &&
      message.type === 'success'
    ) {
      this.#phase = 'running';
      this.#resolveBootstrap();
      return;
    }
    this.#phase = 'failed';
    if (
      typeof message === 'object' &&
      message !== null &&
      'type' in message &&
      message.type === 'error'
    ) {
      this.#rejectBootstrap('error' in message ? message.error : message);
      return;
    }
    this.#rejectBootstrap(new Error('Parallel-plugin worker sent an invalid bootstrap response'));
  };

  readonly #onError = (error: Error) => {
    if (this.#phase === 'bootstrapping') {
      this.#phase = 'failed';
      this.#worker.off('message', this.#onMessage);
      this.#rejectBootstrap(error);
      return;
    }
    if (this.#phase !== 'failed' && this.#phase !== 'stopped') {
      this.#faults.push(error);
    }
  };

  readonly #onExit = (code: number) => {
    this.#exitCode = code;
    if (this.#phase === 'bootstrapping') {
      this.#phase = 'stopped';
      this.#worker.off('message', this.#onMessage);
      this.#disposeListeners();
      this.#rejectBootstrap(
        new Error(
          `Parallel-plugin worker exited before initialization completed (exit code ${code})`,
        ),
      );
      return;
    }
    if (this.#phase === 'running') {
      this.#faults.push(
        new Error(`Parallel-plugin worker exited unexpectedly (exit code ${code})`),
      );
    }
    this.#phase = 'stopped';
    this.#disposeListeners();
  };

  #disposeListeners(): void {
    this.#worker.off('message', this.#onMessage);
    this.#worker.off('error', this.#onError);
    this.#worker.off('exit', this.#onExit);
  }
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
