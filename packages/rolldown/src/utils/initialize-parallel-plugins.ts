import os from 'node:os';
import { MessageChannel, type MessagePort, Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';
import { assertParallelPluginsSupported } from '../plugin/parallel-plugin';
import {
  cleanupAfterError,
  clearRetryableCleanup,
  recoverRetryableCleanups,
  trackRetryableCleanupOwnership,
  type RetryableCleanup,
} from './retryable-cleanup';

export type ParallelPluginWorkerData = {
  registryId: number;
  pluginInfos: ParallelPluginInfo[];
  threadNumber: number;
  watchMode: boolean;
};

type WorkerData = ParallelPluginWorkerData & {
  controlPort: MessagePort;
};

export interface WorkerBootstrapAuthentication {
  readyToken: string;
  resultToken: string;
  session: string;
  startToken: string;
}

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
  on(event: 'message', listener: (message: unknown) => void): this;
  off(event: 'error', listener: (error: Error) => void): this;
  off(event: 'exit', listener: (code: number) => void): this;
  off(event: 'message', listener: (message: unknown) => void): this;
  postMessage(message: unknown): void;
  unref?(): void;
}

interface BootstrapControlPort {
  on(event: 'message', listener: (message: unknown) => void): this;
  off(event: 'message', listener: (message: unknown) => void): this;
  postMessage(message: unknown): void;
  close?(): void;
  unref?(): void;
}

export interface SupervisedWorker extends TerminableWorker {
  startBootstrap(): void;
  waitForBootstrap(): Promise<void>;
  waitForReadiness(): Promise<void>;
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
const FILE_WORKER_INJECTION_FLAGS_WITH_VALUE = new Set([
  '--experimental-loader',
  '--import',
  '--loader',
  '--require',
  '-r',
]);

/** @internal Remove parent invocation modes that are invalid or meaningless for a file worker. */
export function sanitizeFileWorkerExecArgv(execArgv: readonly string[]): string[] {
  const sanitized: string[] = [];
  for (let index = 0; index < execArgv.length; index += 1) {
    const argument = execArgv[index];
    const equalsIndex = argument.indexOf('=');
    const flag = equalsIndex === -1 ? argument : argument.slice(0, equalsIndex);
    if (
      FILE_WORKER_CONTEXT_FLAGS_WITH_VALUE.has(flag) ||
      FILE_WORKER_INJECTION_FLAGS_WITH_VALUE.has(flag)
    ) {
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

/** @internal Create an isolated worker environment without inherited Node preload hooks. */
export function createParallelPluginWorkerEnv(
  source: NodeJS.ProcessEnv = process.env,
): NodeJS.ProcessEnv {
  const env = { ...source };
  for (const key of Object.keys(env)) {
    if (key.toUpperCase() === 'NODE_OPTIONS') {
      delete env[key];
    }
  }
  env.NODE_OPTIONS = '';
  return env;
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

export async function initializeParallelPlugins(
  plugins: RolldownPlugin[],
  watchMode: boolean = false,
): Promise<
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

  // Descriptors can come from older package copies or be constructed directly,
  // so the consuming artifact must enforce its own capability boundary.
  assertParallelPluginsSupported();

  const count = availableParallelism();
  const parallelJsPluginRegistry = new ParallelJsPluginRegistry(count);
  const registryId = parallelJsPluginRegistry.id;
  const workerTerminationBarrier = new WorkerTerminationBarrier(count);
  const workerBootstrapCoordinator = new WorkerBootstrapCoordinator(count);

  const stopWorkers = await initializeWorkerPool<SupervisedWorker>(
    count,
    async (threadNumber, registerWorker) => {
      await initializeWorker(
        registryId,
        pluginInfos,
        threadNumber,
        watchMode,
        registerWorker,
        workerTerminationBarrier,
        workerBootstrapCoordinator,
      );
    },
  );

  return { registry: parallelJsPluginRegistry, stopWorkers };
}

/**
 * @internal Initialize a pool while retaining every worker from construction onward.
 * Every initializer is invoked before failures are observed, so production workers
 * register synchronously after construction and can be terminated on the first failed
 * bootstrap without waiting for an unrelated bootstrap promise to settle.
 * See internal-docs/async-runtime/implementation.md.
 */
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

  const initializations = Array.from({ length: count }, (_, threadNumber) => {
    try {
      return Promise.resolve(initializeWorker(threadNumber, registerWorker));
    } catch (error) {
      return Promise.reject(error);
    }
  });
  const failures: { error: unknown; threadNumber: number }[] = [];
  let remaining = initializations.length;
  let resolveAllInitializations!: () => void;
  let resolveFirstFailure!: () => void;
  const allInitializations = new Promise<void>((resolve) => {
    resolveAllInitializations = resolve;
  });
  const firstFailure = new Promise<void>((resolve) => {
    resolveFirstFailure = resolve;
  });
  if (remaining === 0) {
    resolveAllInitializations();
  }
  const finishInitialization = () => {
    remaining -= 1;
    if (remaining === 0) {
      resolveAllInitializations();
    }
  };
  for (const [threadNumber, initialization] of initializations.entries()) {
    void initialization.then(finishInitialization, (error: unknown) => {
      failures.push({ error, threadNumber });
      resolveFirstFailure();
      finishInitialization();
    });
  }

  const initializationFailed = await Promise.race([
    allInitializations.then(() => false),
    firstFailure.then(() => true),
  ]);
  if (initializationFailed) {
    // Collect other already-settled failures without waiting for a worker whose
    // bootstrap promise may never settle. Later rejections remain observed by
    // the handlers above and are normally caused by terminating that worker.
    await Promise.resolve();
    const errors = failures
      .sort((left, right) => left.threadNumber - right.threadNumber)
      .map(({ error }) => error);
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

/** @internal Create browser-bundle-safe cryptographic tokens for the worker handshake. */
export function createWorkerBootstrapAuthentication(): WorkerBootstrapAuthentication {
  const randomHex = () =>
    Array.from(globalThis.crypto.getRandomValues(new Uint8Array(24)), (byte) =>
      byte.toString(16).padStart(2, '0'),
    ).join('');

  return {
    readyToken: randomHex(),
    resultToken: randomHex(),
    session: randomHex(),
    startToken: randomHex(),
  };
}

/** @internal Build the lexical bootstrap used by production and direct-entry tests. */
export function createParallelPluginWorkerBootstrap(
  workerUrl: string,
  authentication: WorkerBootstrapAuthentication,
): string {
  return `
const { workerData } = require('node:worker_threads');
const controlPort = workerData.controlPort;
const pluginWorkerData = {
  registryId: workerData.registryId,
  pluginInfos: workerData.pluginInfos,
  threadNumber: workerData.threadNumber,
  watchMode: workerData.watchMode,
};
if (!Reflect.deleteProperty(workerData, 'controlPort')) {
  throw new Error('Parallel-plugin worker could not hide its bootstrap control port');
}
const postControlMessage = controlPort.postMessage.bind(controlPort);
const onControlMessage = controlPort.on.bind(controlPort);
const refControlPort = controlPort.ref.bind(controlPort);
const unrefControlPort = controlPort.unref.bind(controlPort);
const closeControlPortHandle = controlPort.close.bind(controlPort);
const authentication = Object.freeze(${JSON.stringify(authentication)});

function readThrownValueDetail(thrownValue) {
  try {
    if (
      thrownValue !== null &&
      (typeof thrownValue === 'object' || typeof thrownValue === 'function') &&
      'message' in thrownValue
    ) {
      const message = thrownValue.message;
      if (typeof message === 'string' && message.length > 0) return message;
    }
  } catch {}
  try {
    const detail = String(thrownValue);
    return detail === '[object Object]' ? undefined : detail;
  } catch {
    return 'a non-coercible thrown value';
  }
}

function createCloneableBootstrapDiagnostic(
  thrownValue,
  prefix = 'Parallel-plugin worker initialization failed',
) {
  const detail = readThrownValueDetail(thrownValue);
  const diagnostic = new Error(detail ? prefix + ': ' + detail : prefix);
  diagnostic.name = 'ParallelPluginBootstrapError';
  try {
    if (thrownValue instanceof Error && typeof thrownValue.stack === 'string') {
      diagnostic.stack = thrownValue.stack;
    }
  } catch {}
  return diagnostic;
}

function disposeControlPort() {
  try {
    unrefControlPort();
    closeControlPortHandle();
  } catch {}
}

function postBootstrapResult(message) {
  try {
    postControlMessage({
      ...message,
      session: authentication.session,
      token: message.type === 'ready' ? authentication.readyToken : authentication.resultToken,
    });
  } catch (postMessageError) {
    disposeControlPort();
    const bootstrapDiagnostic =
      message.type === 'error'
        ? message.error
        : new Error(
            message.type === 'ready'
              ? 'Parallel-plugin worker could not report binding readiness'
              : 'Parallel-plugin worker could not report successful initialization',
          );
    const reportingDiagnostic = createCloneableBootstrapDiagnostic(
      postMessageError,
      'Parallel-plugin worker could not report its bootstrap result',
    );
    const terminalDiagnostic = new Error(
      bootstrapDiagnostic.message + '; ' + reportingDiagnostic.message,
    );
    terminalDiagnostic.name = 'ParallelPluginBootstrapError';
    throw terminalDiagnostic;
  }
}

function waitForStart() {
  return new Promise((resolve, reject) => {
    let started = false;
    const onMessage = (message) => {
      let session;
      let token;
      let type;
      try {
        session = Reflect.get(message, 'session', message);
        token = Reflect.get(message, 'token', message);
        type = Reflect.get(message, 'type', message);
      } catch {
        return;
      }
      if (session !== authentication.session) return;
      if (started || token !== authentication.startToken || type !== 'start') {
        reject(new Error('Parallel-plugin worker received an invalid authenticated start message'));
        return;
      }
      // Keep this authenticated listener installed after start. A MessagePort
      // with no listeners does not retain the worker even after ref(), while
      // plugin callbacks intentionally use weak TSFNs and depend on this port
      // to keep the environment alive until explicit pool shutdown.
      started = true;
      resolve();
    };
    onControlMessage('message', onMessage);
  });
}

void (async () => {
  refControlPort();
  const workerModule = await import(${JSON.stringify(workerUrl)});
  const start = waitForStart();
  postBootstrapResult({ type: 'ready' });
  await start;
  try {
    await workerModule.initializeParallelPluginWorker(pluginWorkerData);
  } catch (error) {
    postBootstrapResult({
      type: 'error',
      error: createCloneableBootstrapDiagnostic(error),
    });
    return;
  }
  postBootstrapResult({ type: 'success' });
})().catch((error) => {
  disposeControlPort();
  const diagnostic = createCloneableBootstrapDiagnostic(
    error,
    'Parallel-plugin worker bootstrap failed',
  );
  queueMicrotask(() => {
    throw diagnostic;
  });
});
`;
}

async function initializeWorker(
  registryId: number,
  pluginInfos: ParallelPluginInfo[],
  threadNumber: number,
  watchMode: boolean,
  registerWorker: (worker: SupervisedWorker) => void,
  terminationBarrier: WorkerTerminationBarrier,
  bootstrapCoordinator: WorkerBootstrapCoordinator,
) {
  const terminationSlot = terminationBarrier.createSlot();
  let supervisorPort: MessagePort | undefined;
  let workerPort: MessagePort | undefined;
  let supervisedWorker: WorkerSupervisor | undefined;
  try {
    const urlString = import.meta.resolve('#parallel-plugin-worker');
    const authentication = createWorkerBootstrapAuthentication();
    const bootstrap = createParallelPluginWorkerBootstrap(urlString, authentication);
    ({ port1: supervisorPort, port2: workerPort } = new MessageChannel());
    const workerData: WorkerData = {
      controlPort: workerPort,
      registryId,
      pluginInfos,
      threadNumber,
      watchMode,
    };
    const worker = new Worker(bootstrap, {
      env: createParallelPluginWorkerEnv(),
      eval: true,
      workerData,
      execArgv: sanitizeFileWorkerExecArgv(process.execArgv),
      transferList: [workerPort],
    });
    supervisedWorker = new WorkerSupervisor(
      worker,
      terminationSlot,
      authentication,
      supervisorPort,
    );
    bootstrapCoordinator.register(threadNumber, supervisedWorker);
    registerWorker(supervisedWorker);

    await supervisedWorker.waitForReadiness();
    const poolStarted = bootstrapCoordinator.markReady(threadNumber);
    await Promise.race([poolStarted, supervisedWorker.waitForBootstrap()]);
    await supervisedWorker.waitForBootstrap();
    worker.unref();
  } finally {
    if (!supervisedWorker) {
      try {
        supervisorPort?.close();
        workerPort?.close();
      } catch {}
      terminationSlot.arrive();
    }
  }
}

/** @internal Retain worker fault supervision from construction through shutdown. */
export function superviseWorker(
  worker: BootstrapWorker,
  authentication: WorkerBootstrapAuthentication,
): SupervisedWorker {
  return new WorkerSupervisor(worker, new WorkerTerminationBarrier(1).createSlot(), authentication);
}

type WorkerPhase = 'bootstrapping' | 'running' | 'failed' | 'stopping' | 'stopped';

class WorkerTerminationBarrier {
  readonly #settled: Promise<void>;
  #resolveSettled!: () => void;
  #remaining: number;

  constructor(count: number) {
    this.#remaining = count;
    this.#settled = new Promise<void>((resolve) => {
      this.#resolveSettled = resolve;
    });
    if (count === 0) {
      this.#resolveSettled();
    }
  }

  createSlot(): WorkerTerminationSlot {
    return new WorkerTerminationSlot(this);
  }

  arrive(): void {
    if (this.#remaining === 0) return;
    this.#remaining -= 1;
    if (this.#remaining === 0) {
      this.#resolveSettled();
    }
  }

  wait(): Promise<void> {
    return this.#settled;
  }
}

class WorkerTerminationSlot {
  readonly #barrier: WorkerTerminationBarrier;
  #arrived = false;

  constructor(barrier: WorkerTerminationBarrier) {
    this.#barrier = barrier;
  }

  arrive(): void {
    if (this.#arrived) return;
    this.#arrived = true;
    this.#barrier.arrive();
  }

  wait(): Promise<void> {
    return this.#barrier.wait();
  }
}

/** @internal Start plugin initialization only after every worker reports binding readiness. */
export class WorkerBootstrapCoordinator {
  readonly #workers: Array<SupervisedWorker | undefined>;
  readonly #started: Promise<void>;
  #resolveStarted!: () => void;
  readonly #ready = new Set<number>();

  constructor(count: number) {
    this.#workers = Array.from({ length: count });
    this.#started = new Promise<void>((resolve) => {
      this.#resolveStarted = resolve;
    });
    if (count === 0) {
      this.#resolveStarted();
    }
  }

  register(threadNumber: number, worker: SupervisedWorker): void {
    if (
      !Number.isSafeInteger(threadNumber) ||
      threadNumber < 0 ||
      threadNumber >= this.#workers.length ||
      this.#workers[threadNumber]
    ) {
      throw new Error(`Invalid parallel-plugin worker registration for thread ${threadNumber}`);
    }
    this.#workers[threadNumber] = worker;
  }

  markReady(threadNumber: number): Promise<void> {
    const worker = this.#workers[threadNumber];
    if (!worker || this.#ready.has(threadNumber)) {
      throw new Error(`Invalid parallel-plugin worker readiness for thread ${threadNumber}`);
    }
    this.#ready.add(threadNumber);
    if (this.#ready.size === this.#workers.length) {
      for (const registeredWorker of this.#workers) {
        if (!registeredWorker) {
          throw new Error('Parallel-plugin worker pool reached readiness with a missing worker');
        }
        registeredWorker.startBootstrap();
      }
      this.#resolveStarted();
    }
    return this.#started;
  }
}

class WorkerSupervisor implements SupervisedWorker {
  readonly #worker: BootstrapWorker;
  readonly #controlPort: BootstrapControlPort;
  readonly #bootstrapPromise: Promise<void>;
  readonly #readinessPromise: Promise<void>;
  readonly #terminationSlot: WorkerTerminationSlot;
  readonly #authentication: WorkerBootstrapAuthentication;
  #resolveBootstrap!: () => void;
  #rejectBootstrap!: (error: unknown) => void;
  #resolveReadiness!: () => void;
  #rejectReadiness!: (error: unknown) => void;
  #terminationBoundaryReached = false;
  #readyReceived = false;
  #startSent = false;
  #terminalReceived = false;
  #activeTermination: Promise<number> | undefined;
  #phase: WorkerPhase = 'bootstrapping';
  #faults: unknown[] = [];
  #exitCode = 0;

  constructor(
    worker: BootstrapWorker,
    terminationSlot: WorkerTerminationSlot,
    authentication: WorkerBootstrapAuthentication,
    controlPort: BootstrapControlPort = worker,
  ) {
    this.#worker = worker;
    this.#controlPort = controlPort;
    this.#authentication = authentication;
    this.#terminationSlot = terminationSlot;
    this.#bootstrapPromise = new Promise<void>((resolve, reject) => {
      this.#resolveBootstrap = resolve;
      this.#rejectBootstrap = reject;
    });
    // Observe early worker failure before waitForBootstrap() is requested.
    // See internal-docs/async-runtime/implementation.md.
    void this.#bootstrapPromise.catch(() => {});
    this.#readinessPromise = new Promise<void>((resolve, reject) => {
      this.#resolveReadiness = resolve;
      this.#rejectReadiness = reject;
    });
    void this.#readinessPromise.catch(() => {});
    controlPort.on('message', this.#onMessage);
    controlPort.unref?.();
    worker.on('error', this.#onError);
    worker.on('exit', this.#onExit);
  }

  waitForBootstrap(): Promise<void> {
    return this.#bootstrapPromise;
  }

  waitForReadiness(): Promise<void> {
    return this.#readinessPromise;
  }

  startBootstrap(): void {
    if (this.#phase !== 'bootstrapping') return;
    if (!this.#readyReceived || this.#startSent || this.#terminalReceived) {
      this.#failProtocol('Parallel-plugin worker could not enter the start phase');
      return;
    }
    this.#startSent = true;
    try {
      this.#controlPort.postMessage({
        session: this.#authentication.session,
        token: this.#authentication.startToken,
        type: 'start',
      });
    } catch (error) {
      this.#phase = 'failed';
      this.#rejectBootstrap(error);
    }
  }

  terminate(): Promise<number> {
    if (this.#activeTermination) return this.#activeTermination;

    const termination = this.#terminate();
    this.#activeTermination = termination;
    void termination.then(
      () => this.#clearActiveTermination(termination),
      () => this.#clearActiveTermination(termination),
    );
    return termination;
  }

  #clearActiveTermination(termination: Promise<number>): void {
    if (this.#activeTermination === termination) {
      this.#activeTermination = undefined;
    }
  }

  async #terminate(): Promise<number> {
    let terminationError: unknown;
    let hasTerminationError = false;
    const previousPhase = this.#phase;
    if (this.#phase !== 'stopped') {
      this.#phase = 'stopping';
      if (previousPhase === 'bootstrapping') {
        const cancellationError = new Error(
          'Parallel-plugin worker initialization was cancelled during pool cleanup',
        );
        this.#rejectReadiness(cancellationError);
        this.#rejectBootstrap(cancellationError);
      }

      // Worker.terminate() may interrupt static native-addon registration and
      // abort the process. The worker reports `ready` after static imports but
      // before plugin initialization, so a never-settling plugin bootstrap is
      // still physically terminable without crossing that N-API boundary.
      await this.#terminationSlot.wait();

      if (!this.#isStopped()) {
        try {
          this.#exitCode = await this.#worker.terminate();
          this.#phase = 'stopped';
          this.#disposeListeners();
        } catch (error) {
          terminationError = error;
          hasTerminationError = true;
          this.#worker.unref?.();
          if (this.#phase !== 'stopped') {
            this.#phase = previousPhase === 'running' ? 'running' : 'failed';
          }
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
    const authenticated = this.#authenticateMessage(message);
    if (!authenticated) return;

    if (authenticated.type === 'ready') {
      if (
        authenticated.token !== 'ready' ||
        (this.#phase !== 'bootstrapping' && this.#phase !== 'stopping') ||
        this.#readyReceived ||
        this.#startSent ||
        this.#terminalReceived
      ) {
        this.#failProtocol('Parallel-plugin worker sent an invalid or duplicate ready message');
        return;
      }
      this.#readyReceived = true;
      this.#markTerminationBoundary();
      this.#resolveReadiness();
      return;
    }

    if (
      authenticated.token !== 'result' ||
      !this.#readyReceived ||
      !this.#startSent ||
      this.#terminalReceived
    ) {
      this.#failProtocol(
        'Parallel-plugin worker sent an out-of-order or duplicate terminal message',
      );
      return;
    }
    this.#terminalReceived = true;
    this.#markTerminationBoundary();
    if (this.#phase !== 'bootstrapping') return;
    if (authenticated.type === 'success') {
      this.#phase = 'running';
      this.#resolveBootstrap();
      return;
    }
    this.#phase = 'failed';
    this.#rejectBootstrap(authenticated.error);
  };

  #authenticateMessage(
    message: unknown,
  ):
    | { type: 'ready'; token: 'ready' | 'result' }
    | { type: 'success'; token: 'ready' | 'result' }
    | { type: 'error'; error: unknown; token: 'ready' | 'result' }
    | undefined {
    if (message === null || (typeof message !== 'object' && typeof message !== 'function')) {
      return undefined;
    }

    let session: unknown;
    let token: unknown;
    let type: unknown;
    try {
      session = Reflect.get(message, 'session', message);
      token = Reflect.get(message, 'token', message);
      type = Reflect.get(message, 'type', message);
    } catch {
      return undefined;
    }
    if (session !== this.#authentication.session || typeof token !== 'string') {
      return undefined;
    }

    const authenticatedToken =
      token === this.#authentication.readyToken
        ? 'ready'
        : token === this.#authentication.resultToken
          ? 'result'
          : undefined;
    if (!authenticatedToken) return undefined;
    if (type === 'ready' || type === 'success') {
      return { type, token: authenticatedToken };
    }
    if (type === 'error') {
      let error: unknown;
      try {
        error = Reflect.get(message, 'error', message);
      } catch (readError) {
        error = readError;
      }
      return { error, type, token: authenticatedToken };
    }
    this.#failProtocol('Parallel-plugin worker sent an invalid authenticated bootstrap response');
    return undefined;
  }

  #failProtocol(message: string): void {
    this.#markTerminationBoundary();
    const error = new Error(message);
    if (this.#phase === 'bootstrapping') {
      this.#phase = 'failed';
      this.#rejectReadiness(error);
      this.#rejectBootstrap(error);
    } else if (this.#phase !== 'stopped') {
      this.#faults.push(error);
    }
  }

  readonly #onError = (error: Error) => {
    if (this.#phase === 'bootstrapping') {
      this.#phase = 'failed';
      this.#stopListeningForBootstrap();
      this.#rejectReadiness(error);
      this.#rejectBootstrap(error);
      return;
    }
    if (this.#phase === 'running') {
      this.#faults.push(error);
      return;
    }
    if (this.#phase === 'stopping' && !this.#terminationBoundaryReached) {
      this.#faults.push(error);
    }
  };

  readonly #onExit = (code: number) => {
    this.#exitCode = code;
    this.#markTerminationBoundary();
    if (this.#phase === 'bootstrapping') {
      this.#phase = 'stopped';
      this.#disposeListeners();
      const exitError = new Error(
        `Parallel-plugin worker exited before initialization completed (exit code ${code})`,
      );
      this.#rejectReadiness(exitError);
      this.#rejectBootstrap(exitError);
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

  #markTerminationBoundary(): void {
    if (this.#terminationBoundaryReached) return;
    this.#terminationBoundaryReached = true;
    this.#terminationSlot.arrive();
  }

  #isStopped(): boolean {
    return this.#phase === 'stopped';
  }

  #stopListeningForBootstrap(): void {
    this.#controlPort.off('message', this.#onMessage);
    this.#controlPort.unref?.();
  }

  #disposeListeners(): void {
    this.#stopListeningForBootstrap();
    this.#worker.off('error', this.#onError);
    this.#worker.off('exit', this.#onExit);
    try {
      this.#controlPort.close?.();
    } catch {}
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
