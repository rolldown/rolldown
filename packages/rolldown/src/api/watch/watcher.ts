import { type BindingWatcherEvent, BindingWatcher, shutdownAsyncRuntime } from '../../binding.cjs';
import { LOG_LEVEL_WARN } from '../../log/logging';
import { logMultipleWatcherOption } from '../../log/logs';
import { aggregateBindingErrorsIntoJsError } from '../../utils/error';
import type { WatchOptions } from '../../options/watch-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import {
  type BundlerOptionWithStopWorker,
  createBundlerOptions,
} from '../../utils/create-bundler-option';
import { arraify } from '../../utils/misc';
import type { WatcherEmitter } from './watch-emitter';

function createEventCallback(
  emitter: WatcherEmitter,
  onNativeClose: () => void,
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
        // The native coordinator awaits this callback. Dispatching listeners
        // here would make a close listener that calls `watcher.close()`
        // self-await the coordinator. Start/observe the JS close lifecycle
        // without awaiting listener dispatch; the public close promise emits
        // after native cleanup and worker termination finish.
        onNativeClose();
        break;
    }
  };
}

class Watcher {
  closed: boolean;
  inner: BindingWatcher;
  emitter: WatcherEmitter;
  stopWorkers: ((() => Promise<void>) | undefined)[];
  nativeClosePromise: Promise<void> | undefined;
  closePromise: Promise<void> | undefined;
  dispatchingCloseEvent: boolean;

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers: ((() => Promise<void>) | undefined)[],
  ) {
    this.closed = false;
    this.dispatchingCloseEvent = false;
    this.inner = inner;
    this.emitter = emitter;
    this.stopWorkers = stopWorkers;

    // Defer so watch() returns the emitter before the first build,
    // giving the caller a chance to attach .on() handlers.
    // This matches Rollup's constructor: process.nextTick(() => this.run())
    process.nextTick(() => this.run());
  }

  close(): Promise<void> {
    // A close listener cannot await the outer promise that is currently
    // awaiting that listener. Native cleanup is already complete at this
    // point, so expose the memoized native phase to reentrant callers.
    if (this.dispatchingCloseEvent) {
      return this.nativeClosePromise ?? Promise.resolve();
    }
    return (this.closePromise ??= this.closeLifecycle());
  }

  onNativeClose(): void {
    // Native close can be observed without a public caller (for example if
    // the coordinator exits independently). Preserve the memoized rejection
    // for a later `close()` call while avoiding an unhandled rejection.
    void this.close().catch(() => {});
  }

  private async closeLifecycle(): Promise<void> {
    this.closed = true;
    this.nativeClosePromise ??= this.inner.close();

    let nativeError: unknown;
    try {
      await this.nativeClosePromise;
    } catch (error) {
      nativeError = error;
    }

    let workerError: unknown;
    try {
      await Promise.all(this.stopWorkers.map(async (stop) => stop?.()));
    } catch (error) {
      workerError = error;
    }

    try {
      if (nativeError !== undefined && workerError !== undefined) {
        throw new AggregateError(
          [nativeError, workerError],
          'Watcher native close and parallel-plugin worker shutdown both failed',
        );
      }
      if (nativeError !== undefined) throw nativeError;
      if (workerError !== undefined) throw workerError;

      this.dispatchingCloseEvent = true;
      await this.emitter.emit('close');
    } finally {
      this.dispatchingCloseEvent = false;
      shutdownAsyncRuntime();
    }
  }

  private async run(): Promise<void> {
    await this.inner.run();
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
  let onNativeClose = () => {};
  const callback = createEventCallback(emitter, () => onNativeClose());
  const bindingWatcher = new BindingWatcher(
    bundlerOptions.map((option) => option.bundlerOptions),
    callback,
  );
  const watcher = new Watcher(
    emitter,
    bindingWatcher,
    bundlerOptions.map((option) => option.stopWorkers),
  );
  emitter.bindClose(() => watcher.close());
  onNativeClose = () => watcher.onNativeClose();
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
