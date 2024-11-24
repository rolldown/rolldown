import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import type { ChangeEvent } from './index'
import { PluginContext } from './plugin-context'
import {
  PluginHookWithBindingExt,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'
import { BindingifyPluginArgs } from './bindingify-plugin'

export function bindingifyWatchChange(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['watchChange']> {
  const hook = args.plugin.watchChange
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, id, event) => {
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        id,
        { event: event as ChangeEvent },
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyCloseWatcher(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeWatcher']> {
  const hook = args.plugin.closeWatcher
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx) => {
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
