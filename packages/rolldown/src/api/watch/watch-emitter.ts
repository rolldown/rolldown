import { BindingWatcherEvent, Bundler } from '../../binding';
import type { MaybePromise } from '../../types/utils';
import { normalizeErrors } from '../../utils/error';

export type WatcherEvent = 'close' | 'event' | 'restart' | 'change';

export type ChangeEvent = 'create' | 'update' | 'delete';

// TODO: find a way use `RolldownBuild` instead of `Bundler`.
export type RolldownWatchBuild = Bundler;

export type RolldownWatcherEvent =
  | { code: 'START' }
  | {
    code: 'BUNDLE_START'; /* input?: InputOption; output: readonly string[] */
  }
  | {
    code: 'BUNDLE_END';
    duration: number;
    // input?: InputOption
    output: readonly string[];
    result: RolldownWatchBuild;
  }
  | { code: 'END' }
  | {
    code: 'ERROR';
    error:
      Error; /* the error is not compilable with rollup * /  /**  result: RollupBuild | null **/
  };

export class WatcherEmitter {
  listeners: Map<
    WatcherEvent,
    Array<(...parameters: any[]) => MaybePromise<void>>
  > = new Map();

  timer: any;

  constructor() {
    // The rust side already create a thread for watcher, but it isn't at main thread.
    // So here we need to avoid main process exit util the user call `watcher.close()`.
    this.timer = setInterval(() => {}, 1e9 /* Low power usage */);
  }

  on(
    event: 'change',
    listener: (
      id: string,
      change: { event: ChangeEvent },
    ) => MaybePromise<void>,
  ): this;
  on(
    event: 'event',
    listener: (data: RolldownWatcherEvent) => MaybePromise<void>,
  ): this;
  on(event: 'restart' | 'close', listener: () => MaybePromise<void>): this;
  on(
    event: WatcherEvent,
    listener: (...parameters: any[]) => MaybePromise<void>,
  ): this {
    const listeners = this.listeners.get(event);
    if (listeners) {
      listeners.push(listener);
    } else {
      this.listeners.set(event, [listener]);
    }
    return this;
  }

  off(
    event: WatcherEvent,
    listener: (...parameters: any[]) => MaybePromise<void>,
  ): this {
    const listeners = this.listeners.get(event);
    if (listeners) {
      const index = listeners.indexOf(listener);
      if (index !== -1) listeners.splice(index, 1);
    }
    return this;
  }

  async onEvent(event: BindingWatcherEvent): Promise<void> {
    const listeners = this.listeners.get(event.eventKind() as WatcherEvent);
    if (listeners) {
      switch (event.eventKind()) {
        case 'close':
        case 'restart':
          for (const listener of listeners) {
            await listener();
          }
          break;

        case 'event':
          for (const listener of listeners) {
            const code = event.bundleEventKind();
            switch (code) {
              case 'BUNDLE_END':
                const { duration, output, result } = event.bundleEndData();
                await listener({
                  code: 'BUNDLE_END',
                  duration,
                  output: [output], // rolldown doesn't support arraying configure output
                  result,
                });
                break;

              case 'ERROR':
                const errors = event.errors();
                await listener({
                  code: 'ERROR',
                  error: normalizeErrors(errors),
                });
                break;

              default:
                await listener({ code });
                break;
            }
          }
          break;

        case 'change':
          for (const listener of listeners) {
            const { path, kind } = event.watchChangeData();
            await listener(path, { event: kind as ChangeEvent });
          }
          break;

        default:
          throw new Error(`Unknown event: ${event}`);
      }
    }
  }

  async close(): Promise<void> {
    clearInterval(this.timer);
  }
}

export type RolldownWatcher = WatcherEmitter;
