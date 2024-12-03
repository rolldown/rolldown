import { BindingWatcher } from '../../binding'
import { WatchOptions } from '../../options/watch-options'
import { createBundler } from '../../utils/create-bundler'
import { WatcherEmitter } from './watch-emitter'

export class Watcher {
  closed: boolean
  controller: AbortController
  inner: BindingWatcher
  emitter: WatcherEmitter
  stopWorkers?: () => Promise<void>

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers?: () => Promise<void>,
  ) {
    this.closed = false
    this.controller = new AbortController()
    this.inner = inner
    this.emitter = emitter
    emitter.close = this.close.bind(this)
    this.stopWorkers = stopWorkers
  }

  async close() {
    if (this.closed) return
    this.closed = true
    await this.stopWorkers?.()
    await this.inner.close()
    this.controller.abort()
  }

  // The rust side already create a thread for watcher, but it isn't at main thread.
  // So here we need to avoid main process exit util the user call `watcher.close()`.
  start() {
    const timer = setInterval(() => {}, 1e9 /* Low power usage */)
    this.controller.signal.addEventListener('abort', () => {
      // eslint-disable-next-line no-console
      console.log('clearInterval')
      clearInterval(timer)
    })
    // run first build after listener is attached
    process.nextTick(() =>
      this.inner.start(this.emitter.onEvent.bind(this.emitter)),
    )
  }
}

export async function createWatcher(
  emitter: WatcherEmitter,
  input: WatchOptions,
) {
  const { bundler, stopWorkers } = await createBundler(
    input,
    input.output || {},
  )
  const bindingWatcher = await bundler.watch()
  const watcher = new Watcher(emitter, bindingWatcher, stopWorkers)
  watcher.start()
}
