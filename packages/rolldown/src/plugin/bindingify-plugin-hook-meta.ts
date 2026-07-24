import { type BindingPluginHookMeta, BindingPluginOrder } from '../binding.cjs';
import type { ObjectHook, ObjectHookMeta, PluginOrder } from '.';
import type { AnyFn } from '../types/utils';
import { normalizeHook } from '../utils/normalize-hook';

export function bindingifyPluginHookMeta(options: ObjectHookMeta): BindingPluginHookMeta {
  return {
    order: bindingPluginOrder(options.order),
  };
}

function bindingPluginOrder(order?: PluginOrder): BindingPluginOrder | undefined {
  switch (order) {
    case 'post':
      return BindingPluginOrder.Post;
    case 'pre':
      return BindingPluginOrder.Pre;
    case null:
    case undefined:
      return undefined;
    default:
      throw new Error(`Unknown plugin order: ${order}`);
  }
}

export type PluginHookWithBindingExt<T, F = undefined> = {
  plugin?: T;
  meta?: BindingPluginHookMeta;
  filter?: F;
};

export function bindingifyHook<Hook extends ObjectHook<AnyFn | string>, T, F = undefined>(
  hook: Hook | undefined,
  build: (normalized: ReturnType<typeof normalizeHook<Hook>>) => { plugin: T; filter?: F },
): PluginHookWithBindingExt<T, F> {
  if (!hook) {
    return {};
  }
  const normalized = normalizeHook(hook);
  return {
    ...build(normalized),
    meta: bindingifyPluginHookMeta(normalized.meta),
  };
}
