import { InputOptions } from './options/input-options'
import { RolldownBuild } from './rolldown-build'

// Compat to `rollup.rollup`, it is include scan module graph and linker.
export const rolldown = (input: InputOptions): Promise<RolldownBuild> => {
  return RolldownBuild.fromInputOptions(input)
}

/**
 * @description
 * This is a experimental API. It's behavior may change in the future.
 * Calling this API will only execute the scan stage of rolldown.
 */
export const experimental_scan = (input: InputOptions): Promise<void> => {
  return RolldownBuild.fromInputOptionsForScanStage(input)
}
