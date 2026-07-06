import type { BindingWatcherBundler } from '../../binding.cjs';
import type { MaybePromise } from '../../types/utils';
import { createAsyncContext } from '../../utils/async-context';
import {
  getRetryableCleanup,
  hasRetryableCleanupOwnership,
  runRetryableCleanup,
  type RetryableCleanup,
} from '../../utils/retryable-cleanup';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { OutputOptions } from '../../options/output-options';

type WatcherEvent = 'close' | 'event' | 'restart' | 'change';

type ChangeEvent = 'create' | 'update' | 'delete';

// TODO: find a way use `RolldownBuild` instead of `Bundler`.
type RolldownWatchBuild = BindingWatcherBundler;

interface ReentrantCloseInvocation {
  active: boolean;
  emitter: WatcherEmitter;
  onReentrantClose?: () => void;
  reentrantClosePromise: Promise<void>;
}

const closeListenerContext = createAsyncContext<ReentrantCloseInvocation>();

/**
 * - `START`: the watcher is (re)starting
 * - `BUNDLE_START`: building an individual bundle
 * - `BUNDLE_END`: finished building a bundle
 *   - `duration`: the build duration in milliseconds
 *   - `output`: an array of the {@linkcode OutputOptions.file | file} or {@linkcode OutputOptions.dir | dir} option values of the generated outputs
 *   - `result`: the bundle object that can be used to generate additional outputs. This is especially important when the watch.skipWrite option is used. You should call `event.result.close()` once you are done generating outputs, or if you do not generate outputs. This will allow plugins to clean up resources via the `closeBundle` hook.
 * - `END`: finished building all bundles
 * - `ERROR`: encountered an error while bundling
 *   - `error`: the error that was thrown
 *   - `result`: the bundle object, or `null` if setup failed before a bundle was created
 *
 * @category Programmatic APIs
 */
export type RolldownWatcherEvent =
  | {
      code: 'START';
    }
  | {
      code: 'BUNDLE_START' /* input?: InputOption; output: readonly string[] */;
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
      error: Error /* the error is not compilable with rollup */;
      result: RolldownWatchBuild | null;
    };

/**
 *
 * @category Programmatic APIs
 */
export type RolldownWatcherWatcherEventMap = {
  event: [data: RolldownWatcherEvent];
  /** a file was modified */
  change: [id: string, change: { event: ChangeEvent }];
  /** a new run was triggered */
  restart: [];
  /** the watcher was closed */
  close: [];
};

/**
 * @category Programmatic APIs
 */
export interface RolldownWatcher {
  /**
   * Register a listener for events defined in {@linkcode RolldownWatcherWatcherEventMap}.
   */
  on<E extends keyof RolldownWatcherWatcherEventMap>(
    event: E,
    listener: (...args: RolldownWatcherWatcherEventMap[E]) => MaybePromise<void>,
  ): this;
  /**
   * Unregister a listener for events defined in {@linkcode RolldownWatcherWatcherEventMap}.
   */
  off<E extends keyof RolldownWatcherWatcherEventMap>(
    event: E,
    listener: (...args: RolldownWatcherWatcherEventMap[E]) => MaybePromise<void>,
  ): this;
  /**
   * Unregister all listeners for a specific event defined in {@linkcode RolldownWatcherWatcherEventMap}.
   */
  clear<E extends keyof RolldownWatcherWatcherEventMap>(event: E): void;
  /**
   * Close the watcher and stop listening for file changes.
   */
  close(): Promise<void>;
}

export class WatcherEmitter implements RolldownWatcher {
  private listeners = new Map<WatcherEvent, Array<(...parameters: any[]) => MaybePromise<void>>>();
  private closeHandlerPromise: Promise<() => Promise<void>>;
  private resolveCloseHandler!: (handler: () => Promise<void>) => void;
  private closeHandler: (() => Promise<void>) | undefined;
  private browserCloseListenerInvocation: ReentrantCloseInvocation | undefined;
  private setupFailureReportCompletion: Promise<void> | undefined;

  constructor() {
    this.closeHandlerPromise = new Promise((resolve) => {
      this.resolveCloseHandler = resolve;
    });
  }

  on(event: WatcherEvent, listener: (...parameters: any[]) => MaybePromise<void>): this {
    const listeners = this.listeners.get(event);
    if (listeners) {
      listeners.push(listener);
    } else {
      this.listeners.set(event, [listener]);
    }
    return this;
  }

  off(event: WatcherEvent, listener: (...parameters: any[]) => MaybePromise<void>): this {
    const listeners = this.listeners.get(event);
    if (listeners) {
      const index = listeners.indexOf(listener);
      if (index !== -1) listeners.splice(index, 1);
    }
    return this;
  }

  clear(event: WatcherEvent): void {
    this.listeners.delete(event);
  }

  /** Async emit — sequential dispatch so side effects from earlier handlers
   *  (e.g. `event.result.close()` triggering `closeBundle`) are visible to later handlers. */
  async emit(event: WatcherEvent, ...args: any[]): Promise<void> {
    const handlers = this.listeners.get(event);
    if (handlers?.length) {
      for (const h of handlers) {
        await h(...args);
      }
    }
  }

  /** @internal Dispatch close listeners with a reentrant close result. */
  async emitClose(reentrantClosePromise: Promise<void>): Promise<void> {
    const handlers = this.listeners.get('close');
    if (!handlers?.length) return;

    const invocation: ReentrantCloseInvocation = {
      active: true,
      emitter: this,
      reentrantClosePromise,
    };
    await this.runWithCloseInvocation(invocation, async () => {
      for (const handler of handlers) {
        await handler();
      }
    });
  }

  private async emitWithCloseInvocation(
    event: WatcherEvent,
    invocation: ReentrantCloseInvocation,
    ...args: any[]
  ): Promise<void> {
    const handlers = this.listeners.get(event);
    if (!handlers?.length) return;
    await this.runWithCloseInvocation(invocation, async () => {
      for (const handler of handlers) {
        await handler(...args);
      }
    });
  }

  private async runWithCloseInvocation(
    invocation: ReentrantCloseInvocation,
    dispatch: () => Promise<void>,
  ): Promise<void> {
    invocation.active = true;
    try {
      if (closeListenerContext) {
        await closeListenerContext.run(invocation, dispatch);
      } else {
        // Browser hosts do not provide async context. Keep the native-phase
        // fallback active across listener awaits to prevent self-deadlock.
        // Calls from unrelated tasks during this window are indistinguishable
        // and receive the same fallback.
        this.browserCloseListenerInvocation = invocation;
        await dispatch();
      }
    } finally {
      invocation.active = false;
      if (this.browserCloseListenerInvocation === invocation) {
        this.browserCloseListenerInvocation = undefined;
      }
    }
  }

  private createSetupFailureClose(cleanup: RetryableCleanup | undefined): () => Promise<void> {
    let closePromise: Promise<void> | undefined;
    let closeEventPromise: Promise<void> | undefined;
    return () => {
      const reentrantClosePromise = this.getReentrantClosePromise();
      if (reentrantClosePromise) return reentrantClosePromise;
      if (!closePromise) {
        closePromise = (async () => {
          const errors: unknown[] = [];
          let retryable = false;
          try {
            if (cleanup && hasRetryableCleanupOwnership(cleanup)) {
              await runRetryableCleanup(cleanup);
            }
          } catch (error) {
            errors.push(error);
            retryable = cleanup !== undefined && hasRetryableCleanupOwnership(cleanup);
          }

          try {
            closeEventPromise ??= this.emitClose(Promise.resolve());
            await closeEventPromise;
          } catch (error) {
            errors.push(error);
          } finally {
            if (retryable) {
              closePromise = undefined;
            }
          }

          if (errors.length === 1) throw errors[0];
          if (errors.length > 1) {
            throw new AggregateError(
              errors,
              'Watcher setup cleanup or close listener dispatch failed',
              { cause: errors[0] },
            );
          }
        })();
      }
      return closePromise;
    };
  }

  close(): Promise<void> {
    const reentrantClosePromise = this.getReentrantClosePromise();
    if (reentrantClosePromise) return reentrantClosePromise;
    // `watch()` returns before createWatcher finishes asynchronous plugin
    // setup. A same-tick close waits for that setup and then enters the same
    // memoized native lifecycle instead of becoming a no-op.
    return this.invokeCloseHandler();
  }

  private getReentrantClosePromise(): Promise<void> | undefined {
    const invocation = closeListenerContext?.getStore() ?? this.browserCloseListenerInvocation;
    if (invocation?.emitter !== this || !invocation.active) return undefined;
    invocation.onReentrantClose?.();
    return invocation.reentrantClosePromise;
  }

  private invokeCloseHandler(): Promise<void> {
    const invoke = (handler: () => Promise<void>) => {
      const reportCompletion = this.setupFailureReportCompletion;
      return reportCompletion ? reportCompletion.then(handler) : handler();
    };
    return this.closeHandler ? invoke(this.closeHandler) : this.closeHandlerPromise.then(invoke);
  }

  /** @internal Bind the native lifecycle after asynchronous option/plugin setup. */
  bindClose(handler: () => Promise<void>): void {
    this.closeHandler = handler;
    this.resolveCloseHandler(handler);
  }

  /** @internal Surface setup failures through the normal watcher event API. */
  failSetup(error: unknown): Promise<void> {
    let resolveReportCompletion!: () => void;
    const reportCompletion = new Promise<void>((resolve) => {
      resolveReportCompletion = resolve;
    });
    this.setupFailureReportCompletion = reportCompletion;

    if (!this.closeHandler) {
      this.bindClose(this.createSetupFailureClose(getRetryableCleanup(error)));
    }

    const normalizedError = normalizeSetupError(error);
    const invocation: ReentrantCloseInvocation = {
      active: true,
      emitter: this,
      onReentrantClose: () => {
        void this.invokeCloseHandler().catch(() => {});
      },
      reentrantClosePromise: Promise.resolve(),
    };
    const reportPromise = (async () => {
      const errors: unknown[] = [];
      try {
        await this.emitWithCloseInvocation('event', invocation, {
          code: 'ERROR',
          error: normalizedError,
          result: null,
        });
      } catch (reportError) {
        errors.push(reportError);
      }
      try {
        await this.emitWithCloseInvocation('event', invocation, { code: 'END' });
      } catch (reportError) {
        errors.push(reportError);
      }
      if (errors.length === 1) throw errors[0];
      if (errors.length > 1) {
        throw new AggregateError(errors, 'Watcher setup terminal event listeners failed', {
          cause: errors[0],
        });
      }
    })().finally(resolveReportCompletion);
    return reportPromise;
  }
}

function normalizeSetupError(error: unknown): Error {
  try {
    if (
      error instanceof Error ||
      (Object.prototype.toString.call(error) === '[object Error]' &&
        typeof (error as Error).message === 'string')
    ) {
      return error as Error;
    }
  } catch {}

  try {
    return new Error(String(error), { cause: error });
  } catch {
    return new Error('Watcher setup failed with a non-coercible thrown value', { cause: error });
  }
}
