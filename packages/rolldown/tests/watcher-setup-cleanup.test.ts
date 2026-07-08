// @ts-nocheck These focused unit tests intentionally reach package source outside the test rootDir.
import { runInNewContext } from 'node:vm';
import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  acquireRuntimeLease: vi.fn(),
  bindingClose: vi.fn(),
  bindingCallback: undefined as unknown,
  bindingConstructionError: undefined as unknown,
  bindingConstructions: 0,
  bindingRun: vi.fn(),
  bindingWaitForClose: vi.fn(),
  callOptionsHook: vi.fn(async (option) => option),
  createBundlerOptions: vi.fn(),
  runtimeCapabilities: {
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
    wasi: false,
    watchSupported: true,
  },
}));

vi.mock('../src/binding.cjs', () => ({
  BindingWatcher: class {
    constructor(_options, callback) {
      mocks.bindingConstructions += 1;
      if (mocks.bindingConstructionError) throw mocks.bindingConstructionError;
      mocks.bindingCallback = callback;
    }

    close() {
      return mocks.bindingClose();
    }

    run() {
      return mocks.bindingRun();
    }

    waitForClose() {
      return mocks.bindingWaitForClose();
    }
  },
  getRuntimeCapabilities: () => mocks.runtimeCapabilities,
}));

vi.mock('../src/plugin/plugin-driver', () => ({
  PluginDriver: {
    callOptionsHook: mocks.callOptionsHook,
  },
}));

vi.mock('../src/runtime-lifecycle', () => {
  const throwCloseErrors = (errors, aggregateMessage) => {
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, aggregateMessage, { cause: errors[0] });
    }
  };
  return {
    acquireRuntimeLease: mocks.acquireRuntimeLease,
    CloseCoordinator: class {
      constructor(aggregateMessage) {
        this.aggregateMessage = aggregateMessage;
      }

      close(attempt) {
        return (this.closePromise ??= Promise.resolve().then(async () => {
          const { errors, retryable } = await attempt();
          if (retryable) this.closePromise = undefined;
          throwCloseErrors(errors, this.aggregateMessage);
        }));
      }
    },
    throwCloseErrors,
  };
});

vi.mock('../src/utils/create-bundler-option', () => ({
  createBundlerOptions: mocks.createBundlerOptions,
}));

import { watch } from '../src/api/watch';
import { WatcherEmitter } from '../src/api/watch/watch-emitter';
import { createWatcher } from '../src/api/watch/watcher';
import {
  createCleanupFailureError,
  getRetryableCleanup,
  hasRetryableCleanupOwnership,
  recoverRetryableCleanups,
} from '../src/utils/retryable-cleanup';

const PUBLIC_SETUP_TIMEOUT = 2_000;

beforeEach(() => {
  mocks.acquireRuntimeLease.mockReset();
  mocks.bindingClose.mockReset().mockResolvedValue({
    errors: [],
    nativeOwnedCloseIdentities: [],
  });
  mocks.bindingCallback = undefined;
  mocks.bindingConstructionError = undefined;
  mocks.bindingConstructions = 0;
  mocks.bindingRun.mockReset().mockResolvedValue(undefined);
  mocks.bindingWaitForClose.mockReset().mockResolvedValue(undefined);
  mocks.callOptionsHook.mockClear();
  mocks.createBundlerOptions.mockReset();
  Object.assign(mocks.runtimeCapabilities, {
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  });
});

test('watcher event dispatch uses a stable listener snapshot', async () => {
  const emitter = new WatcherEmitter();
  const calls: string[] = [];
  const lateListener = vi.fn(() => {
    calls.push('late');
  });
  const removedListener = vi.fn(() => {
    calls.push('removed');
  });
  emitter.on('event', () => {
    calls.push('first');
    emitter.off('event', removedListener);
    emitter.on('event', lateListener);
  });
  emitter.on('event', removedListener);

  await emitter.emit('event', { code: 'START' });
  expect(calls).toEqual(['first', 'removed']);
  expect(lateListener).not.toHaveBeenCalled();

  calls.length = 0;
  await emitter.emit('event', { code: 'END' });
  expect(calls).toEqual(['first', 'late']);
});

test('terminal close clears listeners added after an empty close snapshot', async () => {
  const emitter = new WatcherEmitter();
  const lateListener = vi.fn();

  const closeDispatch = emitter.emitClose(Promise.resolve());
  emitter.on('close', lateListener);
  await closeDispatch;
  await emitter.emit('close');

  expect(lateListener).not.toHaveBeenCalled();
});

test('public watch reports unsupported runtimes without entering setup', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
    watchSupported: false,
  });

  const watcher = watch({ output: {} });
  const events: string[] = [];
  let reportedError: Error | undefined;
  const endPromise = new Promise<void>((resolve) => {
    watcher.on('event', (event) => {
      events.push(event.code);
      if (event.code === 'ERROR') {
        reportedError = event.error;
      } else if (event.code === 'END') {
        resolve();
      }
    });
  });

  await withTimeout(
    Promise.all([endPromise, watcher.close()]),
    'unsupported watcher reporting and close',
  );

  expect(events).toEqual(['ERROR', 'END']);
  expect(reportedError).toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'watch',
  });
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
  await expect(withTimeout(watcher.close(), 'memoized unsupported watcher close')).resolves.toBe(
    undefined,
  );
});

test('unsupported watcher close cannot overtake terminal reporting or deadlock reentrancy', async () => {
  Object.assign(mocks.runtimeCapabilities, {
    target: 'wasi-threads',
    wasi: true,
    watchSupported: false,
  });

  const watcher = watch({ output: {} });
  const order: string[] = [];
  let releaseEndListener!: () => void;
  const endListenerGate = new Promise<void>((resolve) => {
    releaseEndListener = resolve;
  });
  let markEndStarted!: () => void;
  const endStarted = new Promise<void>((resolve) => {
    markEndStarted = resolve;
  });
  let markEndObserved!: () => void;
  const endObserved = new Promise<void>((resolve) => {
    markEndObserved = resolve;
  });

  watcher.on('event', async (event) => {
    order.push(event.code);
    if (event.code === 'ERROR') {
      await watcher.close();
      order.push('ERROR_CLOSE_RESOLVED');
    } else if (event.code === 'END') {
      markEndStarted();
      await endListenerGate;
      order.push('END_LISTENER_FINISHED');
      markEndObserved();
    }
  });
  watcher.on('close', async () => {
    order.push('CLOSE');
    await endObserved;
    order.push('CLOSE_AFTER_END');
  });

  let externalCloseSettled = false;
  const externalClose = watcher.close().finally(() => {
    externalCloseSettled = true;
  });
  await withTimeout(endStarted, 'unsupported watcher END listener start');
  await Promise.resolve();
  expect(externalCloseSettled).toBe(false);
  expect(order).toEqual(['ERROR', 'ERROR_CLOSE_RESOLVED', 'END']);

  releaseEndListener();
  await withTimeout(externalClose, 'unsupported watcher terminal close');

  expect(order).toEqual([
    'ERROR',
    'ERROR_CLOSE_RESOLVED',
    'END',
    'END_LISTENER_FINISHED',
    'CLOSE',
    'CLOSE_AFTER_END',
  ]);
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

  const error = await createWatcher(new WatcherEmitter(), { output: [{}, {}] }).catch(
    (error: unknown) => error,
  );

  expect(error).toBeInstanceOf(AggregateError);
  expect(mocks.callOptionsHook).toHaveBeenCalledOnce();
  expect(fulfilledStopWorkers).toHaveBeenCalledTimes(2);
  expect(rejectedStopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('watcher snapshots output getters before starting option setup', async () => {
  const outputGetterError = new Error('watch output getter failed');
  const laterOption = {
    get output(): never {
      throw outputGetterError;
    },
  };

  await expect(createWatcher(new WatcherEmitter(), [{ output: {} }, laterOption])).rejects.toBe(
    outputGetterError,
  );

  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});

test('watcher snapshots output array elements before starting option setup', async () => {
  const outputGetterError = new Error('watch output array element getter failed');
  const outputs = [{}];
  Object.defineProperty(outputs, 0, {
    get() {
      throw outputGetterError;
    },
  });

  await expect(
    createWatcher(new WatcherEmitter(), [{ output: {} }, { output: outputs }]),
  ).rejects.toBe(outputGetterError);

  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});

test('watcher runs options once per enabled config and delegates config hooks to its first output', async () => {
  const disabledOutputError = new Error('disabled output should not be read');
  const disabled = {
    watch: false,
    get output(): never {
      throw disabledOutputError;
    },
  };
  const firstOutputs = [{ file: 'first-a.js' }, { file: 'first-b.js' }];
  const secondOutputs = [{ file: 'second-a.js' }, { file: 'second-b.js' }];
  const first = { output: firstOutputs };
  const second = { output: secondOutputs };
  const processedFirst = { ...first, processed: 'first' };
  const processedSecond = { ...second, processed: 'second' };
  let activeOptionsHooks = 0;
  let maxActiveOptionsHooks = 0;
  mocks.callOptionsHook.mockImplementation(async (option) => {
    activeOptionsHooks += 1;
    maxActiveOptionsHooks = Math.max(maxActiveOptionsHooks, activeOptionsHooks);
    await Promise.resolve();
    activeOptionsHooks -= 1;
    return option === first ? processedFirst : processedSecond;
  });
  mocks.createBundlerOptions.mockImplementation(async (inputOptions) =>
    createBundlerOption(undefined, inputOptions),
  );
  const release = vi.fn();
  mocks.acquireRuntimeLease.mockResolvedValue({ release });

  const emitter = new WatcherEmitter();
  await createWatcher(emitter, [disabled, first, second]);

  expect(mocks.callOptionsHook.mock.calls.map(([option]) => option)).toEqual([first, second]);
  expect(maxActiveOptionsHooks).toBe(2);
  const firstCalls = mocks.createBundlerOptions.mock.calls.filter(
    ([inputOptions]) => inputOptions === processedFirst,
  );
  const secondCalls = mocks.createBundlerOptions.mock.calls.filter(
    ([inputOptions]) => inputOptions === processedSecond,
  );
  expect(
    firstCalls.map(([, output, watchMode, , configWatchHooks]) => [
      output,
      watchMode,
      configWatchHooks,
    ]),
  ).toEqual([
    [firstOutputs[0], true, true],
    [firstOutputs[1], true, false],
  ]);
  expect(
    secondCalls.map(([, output, watchMode, , configWatchHooks]) => [
      output,
      watchMode,
      configWatchHooks,
    ]),
  ).toEqual([
    [secondOutputs[0], true, true],
    [secondOutputs[1], true, false],
  ]);

  await emitter.close();
  expect(release).toHaveBeenCalledOnce();
});

test('all-disabled watcher setup reports an asynchronous configuration error and remains closable', async () => {
  const disabledOutputError = new Error('disabled output should not be read');
  const watcher = watch({
    watch: false,
    get output(): never {
      throw disabledOutputError;
    },
  });
  const events: string[] = [];
  let reportedError: Error | undefined;
  watcher.on('event', (event) => {
    events.push(event.code);
    if (event.code === 'ERROR') reportedError = event.error;
  });
  watcher.on('close', () => {
    events.push('CLOSE');
  });

  await withTimeout(watcher.close(), 'all-disabled watcher close');

  expect(events).toEqual(['ERROR', 'END', 'CLOSE']);
  expect(reportedError).toBeInstanceOf(TypeError);
  expect(reportedError?.message).toBe(
    'watch() requires at least one configuration with watch enabled',
  );
  expect(mocks.callOptionsHook).not.toHaveBeenCalled();
  expect(mocks.createBundlerOptions).not.toHaveBeenCalled();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
  expect(mocks.bindingConstructions).toBe(0);
});

test('watcher runtime setup retries failed worker cleanup', async () => {
  const setupError = new Error('runtime lease setup failed');
  const cleanupError = new Error('worker cleanup failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockRejectedValue(setupError);

  const error = await createWatcher(new WatcherEmitter(), { output: {} }).catch(
    (error: unknown) => error,
  );

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(setupError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('watcher warning failure cleans every initialized worker pool', async () => {
  const warningError = new Error('watcher warning failed');
  const firstStopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const secondStopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  mocks.createBundlerOptions
    .mockResolvedValueOnce(
      createBundlerOption(firstStopWorkers, {
        watch: { watcher: { usePolling: true } },
      }),
    )
    .mockResolvedValueOnce(
      createBundlerOption(
        secondStopWorkers,
        {
          watch: { watcher: { pollInterval: 100 } },
        },
        () => {
          throw warningError;
        },
      ),
    );

  const error = await createWatcher(new WatcherEmitter(), [{ output: {} }, { output: {} }]).catch(
    (error: unknown) => error,
  );

  expect(error).toBe(warningError);
  expect(firstStopWorkers).toHaveBeenCalledOnce();
  expect(secondStopWorkers).toHaveBeenCalledOnce();
  expect(mocks.acquireRuntimeLease).not.toHaveBeenCalled();
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
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingConstructionError = constructionError;

  const error = await createWatcher(new WatcherEmitter(), { output: {} }).catch(
    (error: unknown) => error,
  );

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors[0]).toBe(constructionError);
  expect(stopWorkers).toHaveBeenCalledTimes(2);
  expect(release).toHaveBeenCalledTimes(2);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('asynchronous native close rejection does not skip remaining watcher cleanup', async () => {
  const nativeCloseError = new Error('native watcher close rejected');
  const closeListenerError = new Error('close listener failed');
  const retainedResultClose = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const release = vi.fn();
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingClose.mockImplementation(async () => {
    await Promise.resolve();
    throw nativeCloseError;
  });
  const emitter = new WatcherEmitter();
  const closeListener = vi.fn(() => {
    throw closeListenerError;
  });
  emitter.on('close', closeListener);
  await createWatcher(emitter, { output: {} });
  await mocks.bindingCallback({
    bundleEndData: () => ({
      closeIdentity: 'retained-current',
      duration: 0,
      output: 'out.js',
      result: { close: retainedResultClose },
      taskIndex: 0,
    }),
    bundleEventKind: () => 'BUNDLE_END',
    eventKind: () => 'event',
  });

  const error = await emitter.close().catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([nativeCloseError, closeListenerError]);
  expect(mocks.bindingClose).toHaveBeenCalledOnce();
  expect(retainedResultClose).toHaveBeenCalledOnce();
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(closeListener).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

test('waitForClose transport rejection enters cleanup and is replayed by close', async () => {
  const waitForCloseError = new Error('waitForClose transport rejected');
  const stopWorkers = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const release = vi.fn();
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  mocks.bindingWaitForClose.mockRejectedValue(waitForCloseError);
  const emitter = new WatcherEmitter();
  const closeEvent = new Promise<void>((resolve) => {
    emitter.on('close', resolve);
  });
  await createWatcher(emitter, { output: {} });

  await withTimeout(closeEvent, 'waitForClose rejection cleanup');
  await expect(emitter.close()).rejects.toBe(waitForCloseError);

  expect(mocks.bindingClose).toHaveBeenCalledOnce();
  expect(stopWorkers).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

test('watch result close memoizes a synchronous native throw', async () => {
  const closeError = new Error('result close threw synchronously');
  const resultClose = vi.fn(() => {
    throw closeError;
  });
  const release = vi.fn();
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(undefined));
  mocks.acquireRuntimeLease.mockResolvedValue({ release });
  const emitter = new WatcherEmitter();
  let result: { close(): Promise<void> } | undefined;
  emitter.on('event', (event) => {
    if (event.code === 'BUNDLE_END') result = event.result;
  });
  await createWatcher(emitter, { output: {} });
  await mocks.bindingCallback({
    bundleEndData: () => ({
      closeIdentity: 'sync-close',
      duration: 0,
      output: 'out.js',
      result: { close: resultClose },
      taskIndex: 0,
    }),
    bundleEventKind: () => 'BUNDLE_END',
    eventKind: () => 'event',
  });

  const firstClose = result!.close();
  const secondClose = result!.close();
  const closeResults = await Promise.allSettled([firstClose, secondClose]);

  expect(secondClose).toBe(firstClose);
  expect(closeResults).toEqual([
    { status: 'rejected', reason: closeError },
    { status: 'rejected', reason: closeError },
  ]);
  expect(resultClose).toHaveBeenCalledOnce();

  await emitter.close();
  expect(release).toHaveBeenCalledOnce();
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
  mocks.acquireRuntimeLease.mockRejectedValue(setupError);

  const error = await createWatcher(new WatcherEmitter(), { output: {} }).catch(
    (error: unknown) => error,
  );

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

test('public watcher close retries cleanup retained after pre-construction setup failure', async () => {
  const setupError = new Error('runtime lease setup failed');
  const firstCleanupError = new Error('first worker cleanup failed');
  const secondCleanupError = new Error('second worker cleanup failed');
  const closeCleanupError = new Error('public close worker cleanup failed');
  const stopWorkers = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(firstCleanupError)
    .mockRejectedValueOnce(secondCleanupError)
    .mockRejectedValueOnce(closeCleanupError)
    .mockResolvedValue(undefined);
  mocks.createBundlerOptions.mockResolvedValue(createBundlerOption(stopWorkers));
  mocks.acquireRuntimeLease.mockRejectedValue(setupError);

  const watcher = watch({ output: {} });
  const events: string[] = [];
  const endPromise = new Promise<void>((resolve) => {
    watcher.on('event', (event) => {
      events.push(event.code);
      if (event.code === 'END') resolve();
    });
  });
  const closeListener = vi.fn();
  watcher.on('close', closeListener);

  await withTimeout(endPromise, 'watcher setup events');
  expect(events).toEqual(['ERROR', 'END']);
  expect(stopWorkers).toHaveBeenCalledTimes(2);

  await expect(withTimeout(watcher.close(), 'first watcher cleanup retry')).rejects.toBe(
    closeCleanupError,
  );
  expect(stopWorkers).toHaveBeenCalledTimes(3);
  expect(closeListener).toHaveBeenCalledOnce();

  await expect(
    withTimeout(watcher.close(), 'second watcher cleanup retry'),
  ).resolves.toBeUndefined();
  expect(stopWorkers).toHaveBeenCalledTimes(4);
  expect(closeListener).toHaveBeenCalledOnce();
});

test('public watcher remains closable when a thrown setup value cannot be coerced', async () => {
  const coercionError = new Error('setup value coercion failed');
  const thrownValue = {
    [Symbol.toPrimitive]() {
      throw coercionError;
    },
  };
  mocks.createBundlerOptions.mockRejectedValue(thrownValue);

  const watcher = watch({ output: {} });
  let reportedError: Error | undefined;
  const endPromise = new Promise<void>((resolve) => {
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        reportedError = event.error;
      } else if (event.code === 'END') {
        resolve();
      }
    });
  });

  const closePromise = watcher.close();
  await withTimeout(
    Promise.all([endPromise, closePromise]),
    'non-coercible watcher setup reporting and close',
  );
  expect(reportedError).toMatchObject({
    cause: thrownValue,
    message: 'Watcher setup failed with a non-coercible thrown value',
  });
  await expect(withTimeout(watcher.close(), 'memoized watcher close')).resolves.toBeUndefined();
});

test('public watcher preserves cross-realm setup error identity', async () => {
  const setupError = runInNewContext('new TypeError("cross-realm setup failed")') as Error;
  mocks.createBundlerOptions.mockRejectedValue(setupError);

  const watcher = watch({ output: {} });
  let reportedError: Error | undefined;
  const endPromise = new Promise<void>((resolve) => {
    watcher.on('event', (event) => {
      if (event.code === 'ERROR') {
        reportedError = event.error;
      } else if (event.code === 'END') {
        resolve();
      }
    });
  });

  await withTimeout(
    Promise.all([endPromise, watcher.close()]),
    'cross-realm watcher setup reporting and close',
  );
  expect(reportedError).toBe(setupError);
});

function createBundlerOption(stopWorkers: () => Promise<void>, inputOptions = {}, onLog = vi.fn()) {
  return {
    bundlerOptions: {},
    inputOptions,
    onLog,
    stopWorkers,
  };
}

function withTimeout<T>(promise: Promise<T>, operation: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${operation} timed out after ${PUBLIC_SETUP_TIMEOUT}ms`));
    }, PUBLIC_SETUP_TIMEOUT);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}
