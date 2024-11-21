import { BindingWatcher } from './binding'
import { MaybePromise } from './types/utils'

export class Watcher {
  closed: boolean
  controller: AbortController
  inner: BindingWatcher
  stopWorkers?: () => Promise<void>
  listeners: Map<
    WatcherEvent,
    Array<(...parameters: any[]) => MaybePromise<void>>
  > = new Map()
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
    const listeners = this.listeners.get(event)
    if (listeners) {
      listeners.push(listener)
    } else {
      this.listeners.set(event, [listener])
    }
    return this
  }

  // The rust side already create a thread for watcher, but it isn't at main thread.
  // So here we need to avoid main process exit util the user call `watcher.close()`.
  watch() {
    const timer = setInterval(() => {}, 1e9 /* Low power usage */)
    this.controller.signal.addEventListener('abort', () => {
      clearInterval(timer)
    })
    // run first build after listener is attached
    process.nextTick(() =>
      this.inner.start(async (event) => {
        const listeners = this.listeners.get(event.eventKind() as WatcherEvent)
        if (listeners) {
          switch (event.eventKind()) {
            case 'close':
            case 'restart':
              for (const listener of listeners) {
                await listener()
              }
              break

            case 'event':
              for (const listener of listeners) {
                const code = event.bundleEventKind()
                switch (code) {
                  case 'BUNDLE_END':
                    const { duration, output } = event.bundleEndData()
                    await listener({
                      code: 'BUNDLE_END',
                      duration,
                      output: [output], // rolldown doesn't support arraying configure output
                    })
                    break

                  case 'ERROR':
                    await listener({
                      code: 'ERROR',
                      error: { message: event.error() },
                    })
                    break

                  default:
                    await listener({ code })
                    break
                }
              }
              break

            case 'change':
              for (const listener of listeners) {
                const { path, kind } = event.watchChangeData()
                await listener(path, { event: kind as ChangeEvent })
              }
              break

            default:
              throw new Error(`Unknown event: ${event}`)
          }
        }
      }),
    )
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
  | {
      code: 'ERROR'
      error: {
        message: string
      } /* the error is not compilable with rollup * /  /**  result: RollupBuild | null **/
    }
