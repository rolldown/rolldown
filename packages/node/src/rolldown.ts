import { InputOptions } from './options/input-options'
import { RolldownBuild } from './rolldown-build'

// Compat to `rollup.rollup`, it is include scan module graph and linker.
export const rolldown = (input: InputOptions): Promise<RolldownBuild> => {
  return RolldownBuild.fromInputOptions(input)
}

// It is only for scan module graph.
export const scan = (input: InputOptions): Promise<void> => {
  return RolldownBuild.fromInputOptionsForScanStage(input)
}
