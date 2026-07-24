import type { BindingPluginOptions } from '../binding.cjs';
import { normalizeHook } from '../utils/normalize-hook';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import {
  bindingifyPluginHookMeta,
  type PluginHookWithBindingExt,
} from './bindingify-plugin-hook-meta';
import type { ChangeEvent } from './index';
import { createPluginContext } from './plugin-context';

export function bindingifyWatchChange(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['watchChange']> {
  const hook = args.plugin.watchChange;
  if (!hook) {
    return {};
  }
  const { handler, meta } = normalizeHook(hook);

  return {
    plugin: async (ctx, id, event) => {
      await handler.call(createPluginContext(args, ctx), id, { event: event as ChangeEvent });
    },
    meta: bindingifyPluginHookMeta(meta),
  };
}

export function bindingifyCloseWatcher(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeWatcher']> {
  const hook = args.plugin.closeWatcher;
  if (!hook) {
    return {};
  }
  const { handler, meta } = normalizeHook(hook);

  return {
    plugin: async (ctx) => {
      await handler.call(createPluginContext(args, ctx));
    },
    meta: bindingifyPluginHookMeta(meta),
  };
}
