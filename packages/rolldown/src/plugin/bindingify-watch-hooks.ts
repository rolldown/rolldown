import type { BindingPluginOptions } from '../binding.cjs';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import type { ChangeEvent } from './index';
import { createPluginContext } from './plugin-context';

export function bindingifyHotUpdate(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['hotUpdate']> {
  return bindingifyHook(args.plugin.hotUpdate, ({ handler }) => ({
    plugin: async (ctx, hookArgs) => {
      const result = await handler.call(createPluginContext(args, ctx), {
        type: hookArgs.kind as ChangeEvent,
        file: hookArgs.file,
        modules: hookArgs.modules,
      });
      return result ?? undefined;
    },
  }));
}

export function bindingifyWatchChange(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['watchChange']> {
  return bindingifyHook(args.plugin.watchChange, ({ handler }) => ({
    plugin: async (ctx, id, event) => {
      await handler.call(createPluginContext(args, ctx), id, { event: event as ChangeEvent });
    },
  }));
}

export function bindingifyCloseWatcher(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeWatcher']> {
  return bindingifyHook(args.plugin.closeWatcher, ({ handler }) => ({
    plugin: async (ctx) => {
      await handler.call(createPluginContext(args, ctx));
    },
  }));
}
