import { asyncFlatten } from './async-flatten'
import type { RolldownPlugin, RolldownOutputPlugin } from '../plugin'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import { ENUMERATED_INPUT_PLUGIN_HOOK_NAMES } from '../constants/plugin'
import { logInputHookInOutputPlugin } from '../log/logs'
import { LogHandler } from '../rollup'
import { LOG_LEVEL_WARN } from '../log/logging'

export const normalizePluginOption: {
  (plugins: InputOptions['plugins']): Promise<RolldownPlugin[]>
  (plugins: OutputOptions['plugins']): Promise<RolldownOutputPlugin[]>
  (plugins: unknown): Promise<any[]>
} = async (plugins: any) => (await asyncFlatten([plugins])).filter(Boolean)

export function checkOutputPluginOption(
  plugins: RolldownOutputPlugin[],
  onLog: LogHandler,
) {
  for (const plugin of plugins) {
    for (const hook of ENUMERATED_INPUT_PLUGIN_HOOK_NAMES) {
      if (hook in plugin) {
        // remove the hook from the plugin if it is not an output plugin hook, avoid the plugin to be called
        // @ts-expect-error Here the plugin typing should be RolldownPlugin
        delete plugin[hook]
        onLog(
          LOG_LEVEL_WARN,
          logInputHookInOutputPlugin(plugin.name ?? 'unknown', hook),
        )
      }
    }
  }
  return plugins
}
