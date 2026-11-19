import {
  type BindingWatcherEvent,
  BindingWatcher,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding.cjs';
import { aggregateBindingErrorsIntoJsError } from '../../utils/error';
import type { WatchOptions } from '../../options/watch-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';
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
  const setupResults = await Promise.allSettled(
    options
      .map((option) =>
        arraify(option.output || {}).map(async (output) => {
          const inputOptions = await PluginDriver.callOptionsHook(option, true);
          return createBundlerOptions(inputOptions, output, true);
        }),
      )
      .flat(),
  );
  const bundlerOptions = setupResults.flatMap((result) =>
    result.status === 'fulfilled' ? [result.value] : [],
  );
  try {
    const failure = setupResults.find((result) => result.status === 'rejected');
    if (failure) throw failure.reason;
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
  } catch (error) {
    // A sibling setup may have started workers before another setup or the constructor failed.
    // Wait for every cleanup, preserving the setup error if a worker also fails to stop.
    await Promise.allSettled(bundlerOptions.map(async (option) => option.stopWorkers?.()));
    throw error;
  }
}
