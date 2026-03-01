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

class Watcher {
  closed: boolean;
  inner: BindingWatcher;
  emitter: WatcherEmitter;
  stopWorkers: ((() => Promise<void>) | undefined)[];

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers: ((() => Promise<void>) | undefined)[],
  ) {
    this.closed = false;
    this.inner = inner;
    this.emitter = emitter;
    const originClose = emitter.close.bind(emitter);
    emitter.close = async () => {
      await this.close();
      originClose();
    };
    this.stopWorkers = stopWorkers;
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    for (const stop of this.stopWorkers) {
      await stop?.();
    }
    await this.inner.close();
    shutdownAsyncRuntime();
  }

  private createEventCallback(): (event: BindingWatcherEvent) => Promise<void> {
    const emitter = this.emitter;
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

  start(): void {
    // run first build after listener is attached
    process.nextTick(async () => {
      await this.inner.start(this.createEventCallback());
      // Pending Promise keeps Node.js event loop alive — no setInterval needed
      this.inner.waitForClose();
    });
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
  const bindingWatcher = new BindingWatcher(bundlerOptions.map((option) => option.bundlerOptions));
  const watcher = new Watcher(
    emitter,
    bindingWatcher,
    bundlerOptions.map((option) => option.stopWorkers),
  );
  watcher.start();
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
