import {
  type BindingWatcherEvent,
  BindingWatcher,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding.cjs';
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
  stopWorkers: ((() => Promise<void>) | undefined)[];

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers: ((() => Promise<void>) | undefined)[],
  ) {
    this.closed = false;
    this.inner = inner;
    this.emitter = emitter;
    startAsyncRuntime();
    const originClose = emitter.close.bind(emitter);
    emitter.close = async () => {
      await this.close();
      originClose();
    };
    this.stopWorkers = stopWorkers;

    // Defer so watch() returns the emitter before the first build,
    // giving the caller a chance to attach .on() handlers.
    // This matches Rollup's constructor: process.nextTick(() => this.run())
    process.nextTick(() => this.run());
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    try {
      for (const stop of this.stopWorkers) {
        await stop?.();
      }
      await this.inner.close();
    } finally {
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
  // One group per config: the outputs of one config share its watch options.
  const bundlerOptionsByConfig = await Promise.all(
    options.map((option) =>
      Promise.all(
        arraify(option.output || {}).map(async (output) => {
          const inputOptions = await PluginDriver.callOptionsHook(option, true);
          return createBundlerOptions(inputOptions, output, true);
        }),
      ),
    ),
  );
  const bundlerOptions = bundlerOptionsByConfig.flat();
  // The `options` hook runs per output and may return different watcher settings,
  // so collapse by value: identical settings count once per config, divergent ones still warn.
  warnMultipleWatcherOptions(
    bundlerOptionsByConfig.flatMap((group) => {
      const seen = new Map<string, BundlerOptionWithStopWorker>();
      for (const option of group) {
        const key = watcherOptionsKey(option);
        if (!seen.has(key)) seen.set(key, option);
      }
      return [...seen.values()];
    }),
  );
  const callback = createEventCallback(emitter);
  const bindingWatcher = new BindingWatcher(
    bundlerOptions.map((option) => option.bundlerOptions),
    callback,
  );
  new Watcher(
    emitter,
    bindingWatcher,
    bundlerOptions.map((option) => option.stopWorkers),
  );
}

function getWatcherOptions(option: BundlerOptionWithStopWorker) {
  const watch = option.inputOptions.watch;
  return watch && typeof watch === 'object' ? watch.watcher : undefined;
}

function watcherOptionsKey(option: BundlerOptionWithStopWorker): string {
  const watcher = getWatcherOptions(option);
  return JSON.stringify([
    watcher?.usePolling,
    watcher?.pollInterval,
    watcher?.compareContentsForPolling,
    watcher?.useDebounce,
    watcher?.debounceDelay,
    watcher?.debounceTickRate,
  ]);
}

function warnMultipleWatcherOptions(bundlerOptions: BundlerOptionWithStopWorker[]) {
  let found = false;
  for (const option of bundlerOptions) {
    const watcher = getWatcherOptions(option);
    // Mirrors selects_watcher_backend in crates/rolldown_binding/src/watcher.rs
    if (
      watcher &&
      (watcher.usePolling != null ||
        watcher.pollInterval != null ||
        watcher.compareContentsForPolling != null ||
        watcher.useDebounce != null ||
        watcher.debounceDelay != null ||
        watcher.debounceTickRate != null)
    ) {
      if (found) {
        option.onLog(LOG_LEVEL_WARN, logMultipleWatcherOption());
        return;
      }
      found = true;
    }
  }
}
