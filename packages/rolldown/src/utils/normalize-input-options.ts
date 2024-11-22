import { getObjectPlugins } from '../plugin/plugin-driver'
import { getLogger, getOnLog } from '../log/logger'
import { LOG_LEVEL_INFO } from '../log/logging'
import type { InputOptions } from '../types/input-options'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { RolldownPlugin } from '..'

export async function normalizeInputOptions(
  config: InputOptions,
  plugins: RolldownPlugin[],
): Promise<NormalizedInputOptions> {
  const logLevel = config.logLevel || LOG_LEVEL_INFO
  const onLog = getLogger(
    getObjectPlugins(plugins),
    getOnLog(config, logLevel),
    logLevel,
  )
  return {
    ...config,
    onLog,
  }
}
