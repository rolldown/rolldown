import { type BindingWatcherEvent, BindingWatcher } from '../../binding.cjs';
import { LOG_LEVEL_WARN } from '../../log/logging';
import { logMultipleWatcherOption } from '../../log/logs';
import { aggregateBindingErrorsIntoJsError } from '../../utils/error';
import type { WatchOptions } from '../../options/watch-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { acquireRuntimeLease, type RuntimeLease } from '../../runtime-lifecycle';
import {
  type BundlerOptionWithStopWorker,
  createBundlerOptions,
} from '../../utils/create-bundler-option';
import { arraify } from '../../utils/misc';
import type { WatcherEmitter } from './watch-emitter';

function createEventCallback(
  emitter: WatcherEmitter,
): (event: BindingWatcherEvent) => Promise<void> {
  return async (event: BindingWatcherEvent) => {
    switch (event.eventKind()) {
      case 'event': {
        const code = event.bundleEventKind();
        if (code === 'BUNDLE_END') {
          const { duration, output, result } = event.bundleEndData();
          await emitter.emit('event', {
            code: 'BUNDLE_END',
            duration,
            output: [output],
            result,
          });
        } else if (code === 'ERROR') {
          const data = event.bundleErrorData();
          await emitter.emit('event', {
            code: 'ERROR',
            error: aggregateBindingErrorsIntoJsError(data.error),
            result: data.result,
          });
        } else {
          await emitter.emit('event', { code: code as 'START' | 'BUNDLE_START' | 'END' });
        }
        break;
      }
      case 'change': {
        const { path, kind } = event.watchChangeData();
        await emitter.emit('change', path, {
          event: kind as 'create' | 'update' | 'delete',
        });
        break;
      }
      case 'restart':
        await emitter.emit('restart');
        break;
      case 'close':
        await emitter.emit('close');
        break;
    }
  };
}

class Watcher {
  closed: boolean;
  inner: BindingWatcher;
  emitter: WatcherEmitter;
  runtimeLease: RuntimeLease;
  stopWorkers: ((() => Promise<void>) | undefined)[];
  /** A close attempt is running right now. */
  #closing: boolean = false;
  /** Parallel-plugin workers are terminated once, even across close retries. */
  #workersStopped: boolean = false;

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    runtimeLease: RuntimeLease,
    stopWorkers: ((() => Promise<void>) | undefined)[],
  ) {
    this.closed = false;
    this.inner = inner;
    this.emitter = emitter;
    this.runtimeLease = runtimeLease;
    const originClose = emitter.close.bind(emitter);
    emitter.close = async () => {
      await this.close();
      originClose();
    };
    this.stopWorkers = stopWorkers;

    // Defer so watch() returns the emitter before the first build,
    // giving the caller a chance to attach .on() handlers.
    // This matches Rollup's constructor: process.nextTick(() => this.run())
    process.nextTick(() =>
      this.run().catch((error) =>
        console.error('watcher cleanup after failed start failed', error),
      ),
    );
  }

  async close(): Promise<void> {
    // Return without awaiting when a close already finished, and also while one
    // is still running: the native `close` event is dispatched from inside that
    // teardown, and its listener is allowed to re-enter close() (rolldown#9462).
    // Awaiting the in-flight attempt there would deadlock the coordinator.
    if (this.closed || this.#closing) return;
    this.#closing = true;
    try {
      if (!this.#workersStopped) {
        for (const stop of this.stopWorkers) {
          await stop?.();
        }
        this.#workersStopped = true;
      }
      // A stopped or failed async runtime rejects this submission. The native
      // watcher then keeps its retained coordinator -- and with it every fs
      // watcher and bundler -- and only releases them once a later `close()`
      // is accepted. So `closed` is latched only after the teardown actually
      // succeeded: latching it up front would make close() a no-op forever and
      // strand those resources with no caller-reachable recovery.
      await this.inner.close();
      this.closed = true;
    } finally {
      // Cleared either way; `closed` is what makes a finished close a no-op, so
      // a rejected teardown stays retryable once the runtime is back.
      this.#closing = false;
      // Lease release is idempotent, so a failed native close cannot leave
      // the runtime lease behind.
      this.runtimeLease.release();
    }
  }

  private async run(): Promise<void> {
    try {
      await this.inner.run();
    } catch (error) {
      // The async runtime can reject the watcher spawn (fallible start).
      // Surface the failure through the normal watcher event API and release
      // owned resources instead of leaving an unhandled rejection behind.
      void this.emitter
        .failSetup(error)
        .catch((reportError) => console.error('watcher setup error listener failed', reportError));
      await this.close();
      return;
    }
    // No `.await`: Create pending Promise to keep Node.js event loop alive
    this.inner.waitForClose();
  }
}

export async function createWatcher(
  emitter: WatcherEmitter,
  input: WatchOptions | WatchOptions[],
): Promise<void> {
  const options = arraify(input);
  const bundlerOptions = await Promise.all(
    options
      .map((option) =>
        arraify(option.output || {}).map(async (output) => {
          const inputOptions = await PluginDriver.callOptionsHook(option, true);
          return createBundlerOptions(inputOptions, output, true);
        }),
      )
      .flat(),
  );
  warnMultiplePollingOptions(bundlerOptions);
  const callback = createEventCallback(emitter);
  let acquiredLease: RuntimeLease | undefined;
  let bindingWatcher: BindingWatcher;
  try {
    acquiredLease = await acquireRuntimeLease();
    bindingWatcher = new BindingWatcher(
      bundlerOptions.map((option) => option.bundlerOptions),
      callback,
    );
  } catch (error) {
    // Setup failure must not abandon the parallel-plugin workers already
    // spawned by createBundlerOptions or an acquired runtime lease.
    const cleanupErrors: unknown[] = [];
    for (const option of bundlerOptions) {
      try {
        await option.stopWorkers?.();
      } catch (cleanupError) {
        cleanupErrors.push(cleanupError);
      }
    }
    try {
      acquiredLease?.release();
    } catch (cleanupError) {
      cleanupErrors.push(cleanupError);
    }
    if (cleanupErrors.length > 0) {
      throw new AggregateError(
        [error, ...cleanupErrors],
        'Watcher construction, parallel-plugin worker cleanup, or runtime release failed',
        { cause: error },
      );
    }
    throw error;
  }
  new Watcher(
    emitter,
    bindingWatcher,
    acquiredLease,
    bundlerOptions.map((option) => option.stopWorkers),
  );
}

function warnMultiplePollingOptions(bundlerOptions: BundlerOptionWithStopWorker[]) {
  let found = false;
  for (const option of bundlerOptions) {
    const watch = option.inputOptions.watch;
    const watcher =
      watch && typeof watch === 'object' ? (watch.watcher ?? watch.notify) : undefined;
    if (watcher && (watcher.usePolling != null || watcher.pollInterval != null)) {
      if (found) {
        option.onLog(LOG_LEVEL_WARN, logMultipleWatcherOption());
        return;
      }
      found = true;
    }
  }
}
