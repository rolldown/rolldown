import { InputOptions } from './options/input-options'
import { RolldownBuild } from './rolldown-build'
import { createBundler } from './utils'

// Compat to `rollup.rollup`, it is included scan module graph and linker.
export const rolldown = async (input: InputOptions): Promise<RolldownBuild> => {
  const bundler = await createBundler(input)
  return new RolldownBuild(bundler)
}

/**
 * @description
 * This is an experimental API. It's behavior may change in the future.
 * Calling this API will only execute the scan stage of rolldown.
 */
export const experimental_scan = async (input: InputOptions): Promise<void> => {
  const bundler = await createBundler(input)
  await bundler.scan()
}
