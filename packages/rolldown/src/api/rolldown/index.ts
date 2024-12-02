import type { InputOptions } from '../../options/input-options'
import { RolldownBuild } from './rolldown-build'

// `async` here is intentional to be compatible with `rollup.rollup`.
export const rolldown = async (input: InputOptions): Promise<RolldownBuild> => {
  return new RolldownBuild(input)
}
