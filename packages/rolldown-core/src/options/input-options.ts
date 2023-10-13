import { InputOptions as RollupInputOptions } from '../rollup-types'
import { InputOptions as BindingInputOptions } from '@rolldown/node-binding'
import path from 'path'
import { normalizePluginOption } from '../utils'
import { createBuildPluginAdapter } from './create-build-plugin-adapter'

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

  // --- Extra

  cwd?: string
}

function normalizeInput(
  input: InputOptions['input'],
): BindingInputOptions['input'] {
  if (input == null) {
    return {}
  } else if (typeof input === 'string') {
    return {
      main: input,
    }
  } else if (Array.isArray(input)) {
    return Object.fromEntries(
      input.map((src) => {
        const name = path.parse(src).name
        return [name, src]
      }),
    )
  } else {
    return input
  }
}

export async function normalizeInputOptions(
  input_opts: InputOptions,
): Promise<BindingInputOptions> {
  const {
    input,
    treeshake,
    // external,
    plugins,
    cwd,
    preserveSymlinks,
    shimMissingExports,
  } = input_opts

  return {
    input: normalizeInput(input),
    treeshake: treeshake,
    // external: normalizeExternal(external),
    plugins: await normalizePlugins(plugins),
    cwd: cwd ?? process.cwd(),
    shimMissingExports: shimMissingExports ?? false,
    // builtins: {},
    preserveSymlinks: preserveSymlinks ?? false,
  }
}

async function normalizePlugins(
  option: InputOptions['plugins'],
): Promise<BindingInputOptions['plugins']> {
  const plugins = await normalizePluginOption(option)
  const adapters = plugins.map((plugin) => createBuildPluginAdapter(plugin))
  return adapters
}
