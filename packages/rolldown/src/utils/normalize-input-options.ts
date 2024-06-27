import { getObjectPlugins } from '../plugin/plugin-driver'
import { getLogger, getOnLog } from '../log/logger'
import { LOG_LEVEL_INFO } from '../log/logging'
import type { InputOptions } from '../options/input-options'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import { normalizePluginOption } from './normalize-plugin-option'
import { normalizeTreeshakeOptions } from './normalize-tree-shake'

export async function normalizeInputOptions(
  config: InputOptions,
): Promise<NormalizedInputOptions> {
  const { input, ...rest } = config
  const plugins = await normalizePluginOption(config.plugins)
  const treeshake = normalizeTreeshakeOptions(config.treeshake)
  const logLevel = config.logLevel || LOG_LEVEL_INFO
  const onLog = getLogger(
    getObjectPlugins(plugins),
    getOnLog(config, logLevel),
    logLevel,
  )
  return {
    ...rest,
    input: input ? (typeof input === 'string' ? [input] : input) : [],
    plugins,
    logLevel,
    onLog,
    treeshake,
  }
}
