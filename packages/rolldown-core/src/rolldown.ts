import { InputOptions } from './options/input-options'
import { RolldownBuild } from './rolldown-build'

export const rolldown = (input: InputOptions): Promise<RolldownBuild> => {
  return RolldownBuild.fromInputOptions(input)
}
