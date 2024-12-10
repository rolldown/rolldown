import { BindingWatcher } from '../../binding'
import { WatchOptions } from '../../options/watch-options'
import { createBundlerOptions } from '../../utils/create-bundler-option'
import { WatcherEmitter } from './watch-emitter'

export class Watcher {
  closed: boolean
  inner: BindingWatcher
  emitter: WatcherEmitter
  stopWorkers: ((() => Promise<void>) | undefined)[]

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers: ((() => Promise<void>) | undefined)[],
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

  async close(): Promise<void> {
    if (this.closed) return
    this.closed = true
    for (const stop of this.stopWorkers) {
      await stop?.()
    }
    await this.inner.close()
  }

  start(): void {
    // run first build after listener is attached
    process.nextTick(() =>
      this.inner.start(this.emitter.onEvent.bind(this.emitter)),
    )
  }
}

export async function createWatcher(
  emitter: WatcherEmitter,
  input: WatchOptions | WatchOptions[],
): Promise<void> {
  const options = Array.isArray(input) ? input : [input]
  const bundlerOptions = await Promise.all(
    options.map((option) => createBundlerOptions(option, option.output || {})),
  )
  const bindingWatcher = new BindingWatcher(
    bundlerOptions.map((option) => option.bundlerOptions),
  )
  const watcher = new Watcher(
    emitter,
    bindingWatcher,
    bundlerOptions.map((option) => option.stopWorkers),
  )
  watcher.start()
}
