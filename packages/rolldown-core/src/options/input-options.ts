import {
  NormalizedInputOptions,
  InputOptions as RollupInputOptions,
} from '../rollup-types'
import { normalizePluginOption } from '../utils'

export interface InputOptions extends RollupInputOptions {
  // --- NotGoingToSupports

  /**
   * @deprecated
   * Rolldown use SWC to parse code
   */
  acorn?: never
  /**
   * @deprecated
   * Rolldown use SWC to parse code
   */
  acornInjectPlugins?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  cache?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  experimentalCacheExpiry?: never
  /**
   * @deprecated
   * deprecated by Rollup
   */
  inlineDynamicImports?: never
  /**
   * @deprecated
   * deprecated by Rollup
   */
  manualChunks?: never
  /**
   * @deprecated
   * TODO: Need to investigate.
   */
  maxParallelFileOps?: never
  /**
   * @deprecated
   * deprecated by Rollup
   */
  maxParallelFileReads?: never
  /**
   * @deprecated
   * TODO: Need to investigate.
   */
  onwarn?: never
  /**
   * @deprecated
   * TODO: Need to investigate.
   */
  perf?: never
  /**
   * @deprecated
   * TODO: Need to investigate.
   */
  preserveEntrySignatures?: never
  /**
   * @deprecated
   * deprecated by Rollup
   */
  preserveModules?: never
  /**
   * @deprecated
   * TODO: Need to investigate.
   */
  strictDeprecations?: never
  /**
   * @deprecated
   * TODO: Need to investigate.
   */
  watch?: never

  // --- ToBeSupported

  context?: never
  makeAbsoluteExternalsRelative?: never
  moduleContext?: never

  // --- Rewritten

  treeshake?: boolean
}

export async function normalizeInputOptions(
  config: InputOptions,
): Promise<NormalizedInputOptions> {
  // @ts-expect-error
  return {
    input: getInput(config),
    plugins: await normalizePluginOption(config.plugins),
  }
}

function getInput(config: InputOptions): NormalizedInputOptions['input'] {
  const configInput = config.input
  return configInput == null
    ? []
    : typeof configInput === 'string'
    ? [configInput]
    : configInput
}
