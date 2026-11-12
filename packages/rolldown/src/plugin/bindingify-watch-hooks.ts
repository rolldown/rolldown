import type { BindingPluginOptions } from '../binding.cjs';
import { discardResult, isPromiseLike } from '../utils/misc';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import type { ChangeEvent } from './index';
import { createPluginContext } from './plugin-context';

export function bindingifyHotUpdate(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['hotUpdate']> {
  return bindingifyHook(args.plugin.hotUpdate, ({ handler }) => ({
    plugin: (ctx, hookArgs) => {
      const result = handler.call(createPluginContext(args, ctx), {
        type: hookArgs.kind as ChangeEvent,
        file: hookArgs.file,
        modules: hookArgs.modules,
      });
      if (isPromiseLike(result)) {
        return Promise.resolve(result).then((result) => result ?? undefined);
      }
      return result ?? undefined;
    },
  }));
}

export function bindingifyWatchChange(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['watchChange']> {
  return bindingifyHook(args.plugin.watchChange, ({ handler }) => ({
    plugin: (ctx, id, event) => {
      return discardResult(
        handler.call(createPluginContext(args, ctx), id, { event: event as ChangeEvent }),
      );
    },
  }));
}

export function bindingifyCloseWatcher(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeWatcher']> {
  return bindingifyHook(args.plugin.closeWatcher, ({ handler }) => ({
    plugin: (ctx) => {
      return discardResult(handler.call(createPluginContext(args, ctx)));
    },
  }));
}
