import { spawn } from 'node:child_process'
import { BindingWatcher, BindingWatcherEvent } from './binding'
import { MaybePromise } from './types/utils'

export class Watcher {
  closed: boolean
  controller: AbortController
  inner: BindingWatcher
  stopWorkers?: () => Promise<void>
  constructor(inner: BindingWatcher, stopWorkers?: () => Promise<void>) {
    this.closed = false
    this.controller = new AbortController()
    this.inner = inner
    this.stopWorkers = stopWorkers
  }

  async close() {
    this.closed = true
    await this.stopWorkers?.()
    await this.inner.close()
    this.controller.abort()
  }

  on(
    event: 'change',
    listener: (
      id: string,
      change: { event: ChangeEvent },
    ) => MaybePromise<void>,
  ): this
  on(
    event: 'event',
    listener: (data: RollupWatcherEvent) => MaybePromise<void>,
  ): this
  on(event: 'restart' | 'close', listener: () => MaybePromise<void>): this
  on(
    event: WatcherEvent,
    listener: (...parameters: any[]) => MaybePromise<void>,
  ): this {
    switch (event) {
      case 'close':
        this.inner.on(BindingWatcherEvent.Close, async () => {
          await listener()
        })
        break
      case 'event':
        this.inner.on(BindingWatcherEvent.Event, async (data) => {
          if (data!.code === 'BUNDLE_END') {
            await listener({
              code: 'BUNDLE_END',
              duration: Number(data!.duration),
              output: [data!.output], // rolldown doesn't support arraying configure output
            })
          } else {
            await listener(data)
          }
        })
        break

      case 'restart':
        this.inner.on(BindingWatcherEvent.ReStart, async () => {
          await listener()
        })
        break

      case 'change':
        this.inner.on(BindingWatcherEvent.Change, async (data) => {
          await listener(data!.id, { event: data!.kind as ChangeEvent })
        })
        break
      default:
        throw new Error(`Unknown event: ${event}`)
    }
    return this
  }

  // The rust side already create a thread for watcher, but it isn't at main thread.
  // So here we need to spawn a process to avoid main process exit util the user call `watcher.close()`.
  watch() {
    const watcherWorkerPath = require.resolve('rolldown/watcher-worker')
    const child = spawn(process.argv[0], [watcherWorkerPath], {
      signal: this.controller.signal,
    })
    child.on('error', () => {
      /* ignore AbortError */
    })
  }
}

export type WatcherEvent = 'close' | 'event' | 'restart' | 'change'

export type ChangeEvent = 'create' | 'update' | 'delete'

export type RollupWatcherEvent =
  | { code: 'START' }
  | {
      code: 'BUNDLE_START' /* input?: InputOption; output: readonly string[] */
    }
  | {
      code: 'BUNDLE_END'
      duration: number
      // input?: InputOption
      output: readonly string[]
      // result: RollupBuild
    }
  | { code: 'END' }
  | { code: 'ERROR' /** error: RollupError; result: RollupBuild | null **/ }
