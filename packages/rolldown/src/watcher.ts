import { spawn } from 'node:child_process'
import { resolve } from 'node:path'
import { BindingWatcher } from './binding'

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
