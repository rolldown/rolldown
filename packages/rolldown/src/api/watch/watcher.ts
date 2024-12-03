import { BindingWatcher } from '../../binding'
import { WatchOptions } from '../../options/watch-options'
import { createBundler } from '../../utils/create-bundler'
import { WatcherEmitter } from './watch-emitter'

export class Watcher {
  closed: boolean
  inner: BindingWatcher
  emitter: WatcherEmitter
  stopWorkers?: () => Promise<void>

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers?: () => Promise<void>,
  ) {
    this.closed = false
    this.inner = inner
    this.emitter = emitter
    const originClose = emitter.close.bind(emitter)
    emitter.close = async () => {
      await this.close()
      originClose()
    }
    this.stopWorkers = stopWorkers
  }

  async close() {
    if (this.closed) return
    this.closed = true
    await this.stopWorkers?.()
    await this.inner.close()
  }

  start() {
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
