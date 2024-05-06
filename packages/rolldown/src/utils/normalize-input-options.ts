import type { InputOptions } from '../options/input-options'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import { ensureArray } from './ensure-array'
import { normalizePluginOption } from './normalize-plugin-option'
export async function normalizeInputOptions(
  config: InputOptions,
): Promise<NormalizedInputOptions> {
  const { input, ...rest } = config
  return {
    ...rest,
    input: input ? (typeof input === 'string' ? [input] : input) : [],
    plugins: await normalizePluginOption(config.plugins),
  }
}
