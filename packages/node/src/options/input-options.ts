import {
  NormalizedInputOptions,
  InputOptions as RollupInputOptions,
} from '../rollup-types'
import { ensureArray, normalizePluginOption } from '../utils'

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
    external: getIdMatcher(config.external),
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

const getIdMatcher = <T extends Array<any>>(
  option:
    | undefined
    // | boolean
    | string
    | RegExp
    | (string | RegExp)[]
    | ((id: string, ...parameters: T) => boolean | null | void),
): ((id: string, ...parameters: T) => boolean) => {
  // if (option === true) {
  // 	return () => true;
  // }
  if (typeof option === 'function') {
    return (id, ...parameters) =>
      (!id.startsWith('\0') && option(id, ...parameters)) || false
  }
  if (option) {
    const ids = new Set<string>()
    const matchers: RegExp[] = []
    for (const value of ensureArray(option)) {
      if (value instanceof RegExp) {
        matchers.push(value)
      } else {
        ids.add(value)
      }
    }
    return (id: string, ..._arguments) =>
      ids.has(id) || matchers.some((matcher) => matcher.test(id))
  }
  // Rollup here convert `undefined` to function, it is bad for performance. So it will convert to `undefined` at adapter.
  return () => false
}
