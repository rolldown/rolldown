import type { InputOptions } from '../../options/input-options'
import { PluginDriver } from '../../plugin/plugin-driver'
import { RolldownBuild } from './rolldown-build'

// `async` here is intentional to be compatible with `rollup.rollup`.
export const rolldown = async (input: InputOptions): Promise<RolldownBuild> => {
  const inputOptions = await PluginDriver.callOptionsHook(input)
  return new RolldownBuild(inputOptions)
}
