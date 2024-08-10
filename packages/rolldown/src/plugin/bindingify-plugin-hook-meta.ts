import { ObjectHookMeta, PluginOrder } from '.'
import { BindingPluginHookMeta, BindingPluginOrder } from '../binding'

export function bindingifyPluginHookMeta(
  options: ObjectHookMeta,
): BindingPluginHookMeta {
  return {
    order: bindingPluginOrder(options.order),
  }
}

function bindingPluginOrder(
  order?: PluginOrder,
): BindingPluginOrder | undefined {
  switch (order) {
    case 'post':
      return BindingPluginOrder.Post
    case 'pre':
      return BindingPluginOrder.Pre
    case null:
    case undefined:
      return undefined
    default:
      throw new Error(`Unknown plugin order: ${order}`)
  }
}

export type PluginHookWithBindingMeta<T> = [
  T | undefined,
  BindingPluginHookMeta | undefined,
]
