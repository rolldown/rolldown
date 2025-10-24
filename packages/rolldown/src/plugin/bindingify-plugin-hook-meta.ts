import { type BindingPluginHookMeta, BindingPluginOrder } from '../binding.cjs';
import type { ObjectHookMeta, PluginOrder } from '.';

export function bindingifyPluginHookMeta(
  options: ObjectHookMeta,
): BindingPluginHookMeta {
  return {
    order: bindingPluginOrder(options.order),
  };
}

function bindingPluginOrder(
  order?: PluginOrder,
): BindingPluginOrder | undefined {
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
