import { BindingWatcher, shutdownAsyncRuntime } from '../../binding';
import { LOG_LEVEL_WARN } from '../../log/logging';
import { logMultiplyNotifyOption } from '../../log/logs';
import { WatchOptions } from '../../options/watch-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import {
  BundlerOptionWithStopWorker,
  createBundlerOptions,
} from '../../utils/create-bundler-option';
import { arraify } from '../../utils/misc';
import { WatcherEmitter } from './watch-emitter';

export class Watcher {
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

  start(): void {
    // run first build after listener is attached
    process.nextTick(() =>
      this.inner.start(this.emitter.onEvent.bind(this.emitter))
    );
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
        })
      )
      .flat(),
  );
  const notifyOptions = getValidNotifyOption(bundlerOptions);
  const bindingWatcher = new BindingWatcher(
    bundlerOptions.map((option) => option.bundlerOptions),
    notifyOptions,
  );
  const watcher = new Watcher(
    emitter,
    bindingWatcher,
    bundlerOptions.map((option) => option.stopWorkers),
  );
  watcher.start();
}

function getValidNotifyOption(bundlerOptions: BundlerOptionWithStopWorker[]) {
  let result;
  for (const option of bundlerOptions) {
    if (option.inputOptions.watch) {
      const notifyOption = option.inputOptions.watch.notify;
      if (notifyOption) {
        if (result) {
          option.onLog(LOG_LEVEL_WARN, logMultiplyNotifyOption());
          return result;
        } else {
          result = notifyOption;
        }
      }
    }
  }
}
