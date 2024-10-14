import type { InputOptions } from './options/input-options'
import { RolldownBuild } from './rolldown-build'
import { Watcher } from './watcher'
import { createBundler } from './utils/create-bundler'

// Compat to `rollup.rollup`, it is included scan module graph and linker.
export const rolldown = async (input: InputOptions): Promise<RolldownBuild> => {
  return new RolldownBuild(input)
}

// Compat to `rollup.watch`
export const watch = async (input: InputOptions): Promise<Watcher> => {
  return new RolldownBuild(input).watch()
}

/**
 * @description
 * This is an experimental API. It's behavior may change in the future.
 * Calling this API will only execute the scan stage of rolldown.
 */
export const experimental_scan = async (input: InputOptions): Promise<void> => {
  const { bundler, stopWorkers } = await createBundler(input, {})
  await bundler.scan()
  await stopWorkers?.()
}
