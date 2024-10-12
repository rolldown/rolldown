import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { ChangeEvent, Plugin } from './index'
import { PluginContext } from './plugin-context'
import { PluginContextData } from './plugin-context-data'
import {
  PluginHookWithBindingExt,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'

export function bindingifyWatchChange(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['watchChange']> {
  const hook = plugin.watchChange
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, id, event) => {
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        id,
        { event: event as ChangeEvent },
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
