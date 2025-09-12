import { BuiltinPlugin } from '../builtin-plugin/utils';
import { ENUMERATED_INPUT_PLUGIN_HOOK_NAMES } from '../constants/plugin';
import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN } from '../log/logging';
import { logInputHookInOutputPlugin } from '../log/logs';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import type { RolldownOutputPlugin, RolldownPlugin } from '../plugin';
import { asyncFlatten } from './async-flatten';

export const normalizePluginOption: {
  (plugins: OutputOptions['plugins']): Promise<RolldownOutputPlugin[]>;
  (plugins: InputOptions['plugins']): Promise<RolldownPlugin[]>;
  (plugins: unknown): Promise<any[]>;
} = async (plugins: any) => (await asyncFlatten([plugins])).filter(Boolean);

export function checkOutputPluginOption(
  plugins: RolldownOutputPlugin[],
  onLog: LogHandler,
): RolldownOutputPlugin[] {
  for (const plugin of plugins) {
    for (const hook of ENUMERATED_INPUT_PLUGIN_HOOK_NAMES) {
      if (hook in plugin) {
        // remove the hook from the plugin if it is not an output plugin hook, avoid the plugin to be called
        // @ts-expect-error Here the plugin typing should be RolldownPlugin
        delete plugin[hook];
        onLog(LOG_LEVEL_WARN, logInputHookInOutputPlugin(plugin.name!, hook));
      }
    }
  }
  return plugins;
}

export function normalizePlugins<T extends RolldownPlugin>(
  plugins: T[],
  anonymousPrefix: string,
): T[] {
  for (const [index, plugin] of plugins.entries()) {
    if ('_parallel' in plugin) {
      continue;
    }
    if (plugin instanceof BuiltinPlugin) {
      continue;
    }
    if (!plugin.name) {
      plugin.name = `${anonymousPrefix}${index + 1}`;
    }
  }
  return plugins;
}

export const ANONYMOUS_PLUGIN_PREFIX = 'at position ';
export const ANONYMOUS_OUTPUT_PLUGIN_PREFIX = 'at output position ';
