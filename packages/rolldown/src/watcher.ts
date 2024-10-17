import { spawn } from 'node:child_process'
import { resolve } from 'node:path'
import { BindingWatcher, BindingWatcherEvent } from './binding'

export class Watcher {
  closed: boolean
  controller: AbortController
  inner: BindingWatcher
  constructor(inner: BindingWatcher) {
    this.closed = false
    this.controller = new AbortController()
    this.inner = inner
  }

  async close() {
    this.closed = true
    await this.inner.close()
    this.controller.abort()
  }

  async on(
    event: 'change',
    listener: (
      id: string,
      change: { event: ChangeEvent },
    ) => void | Promise<void>,
  ): Promise<void>
  async on(
    event: 'event',
    listener: (data: RollupWatcherEvent) => void | Promise<void>,
  ): Promise<void>
  async on(
    event: 'restart' | 'close',
    listener: () => void | Promise<void>,
  ): Promise<void>
  async on(
    event: WatcherEvent,
    listener: (...parameters: any[]) => void | Promise<void>,
  ): Promise<void> {
    switch (event) {
      case 'close':
        return await this.inner.on(BindingWatcherEvent.Close, async () => {
          await listener()
        })
      case 'event':
        return await this.inner.on(BindingWatcherEvent.Event, async (data) => {
          await listener(data)
        })
      case 'restart':
        return await this.inner.on(BindingWatcherEvent.ReStart, async () => {
          await listener()
        })
      case 'change':
        return await this.inner.on(BindingWatcherEvent.Change, async (data) => {
          await listener(data!.id, { event: data!.kind as ChangeEvent })
        })
      default:
        throw new Error(`Unknown event: ${event}`)
    }
  }

  // The rust side already create a thread for watcher, but it isn't at main thread.
  // So here we need to spawn a process to avoid main process exit util the user call `watcher.close()`.
  watch() {
    const watcherWorkerPath = resolve('rolldown', './watcher-worker.js')
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
      // duration: number
      // input?: InputOption
      // output: readonly string[]
      // result: RollupBuild
    }
  | { code: 'END' }
  | { code: 'ERROR' /** error: RollupError; result: RollupBuild | null **/ }
