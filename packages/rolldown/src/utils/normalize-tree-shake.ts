import type { OutputOptions, OutputPlugin } from '../rollup-types'
import type { InputOptions } from '../options/input-options'
import { asyncFlatten } from './async-flatten'
import type { ParallelPlugin, Plugin } from '../plugin'
import { NormalizedTreeshakingOptions } from '@src/treeshake'


export async function normalizeTreeshakeOptions(
  config: InputOptions['treeshake'],
): Promise<NormalizedTreeshakingOptions | undefined> {
  if (config === false) {
    return undefined
  }
  if (config === true || config === undefined) {
    return {
      moduleSideEffects: true
    }
  }
  if (config.moduleSideEffects === undefined) {
    config.moduleSideEffects = true;
  }
  return config
}
