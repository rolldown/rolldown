import { getLogger, getOnLog } from '../log/logger'
import { LOG_LEVEL_INFO } from '../log/logging'
import type { InputOptions } from '../options/input-options'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import { normalizePluginOption } from './normalize-plugin-option'

export async function normalizeInputOptions(
  config: InputOptions,
): Promise<NormalizedInputOptions> {
  const { input, ...rest } = config
  const plugins = await normalizePluginOption(config.plugins)
  const logLevel = config.logLevel || LOG_LEVEL_INFO
  const onLog = getLogger(
    plugins.filter((plugin) => !('_parallel' in plugin)) as Plugin[],
    getOnLog(config, logLevel),
    logLevel,
  )
  return {
    ...rest,
    input: input ? (typeof input === 'string' ? [input] : input) : [],
    plugins,
    logLevel,
    onLog,
  }
}
