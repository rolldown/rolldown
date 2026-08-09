import type { BindingPluginOptions } from '../binding.cjs';
import { releaseOrDefer } from '../utils/threadless-free';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import type { ChangeEvent } from './index';
import { createPluginContext } from './plugin-context';

// Watch mode never runs on the threadless-WASI flavor today, but these
// wrappers release their per-invocation `BindingPluginContext` boxes on that
// flavor anyway, for consistency with the build/output hook wrappers (see the
// comment in `bindingify-build-hooks.ts`).
export function bindingifyHotUpdate(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['hotUpdate']> {
  return bindingifyHook(args.plugin.hotUpdate, ({ handler }) => ({
    plugin: async (ctx, hookArgs) => {
      try {
        const result = await handler.call(createPluginContext(args, ctx), {
          type: hookArgs.kind as ChangeEvent,
          file: hookArgs.file,
          modules: hookArgs.modules,
        });
        return result ?? undefined;
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyWatchChange(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['watchChange']> {
  return bindingifyHook(args.plugin.watchChange, ({ handler }) => ({
    plugin: async (ctx, id, event) => {
      try {
        await handler.call(createPluginContext(args, ctx), id, { event: event as ChangeEvent });
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyCloseWatcher(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeWatcher']> {
  return bindingifyHook(args.plugin.closeWatcher, ({ handler }) => ({
    plugin: async (ctx) => {
      try {
        await handler.call(createPluginContext(args, ctx));
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}
