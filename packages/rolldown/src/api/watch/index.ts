import type { WatchOptions } from '../../options/watch-options'
import { createBundler } from '../../utils/create-bundler'
import { Watcher } from './watcher'

// Compat to `rollup.watch`
export const watch = async (input: WatchOptions): Promise<Watcher> => {
  const { bundler, stopWorkers } = await createBundler(
    input,
    input.output || {},
  )
  const bindingWatcher = await bundler.watch()
  const watcher = new Watcher(bindingWatcher, stopWorkers)
  watcher.watch()
  return watcher
}
