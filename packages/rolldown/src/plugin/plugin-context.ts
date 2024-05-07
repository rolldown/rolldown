import { BindingPluginContext } from '../binding'

export interface PluginContext {}

export function transformPluginContext(
  context: BindingPluginContext,
): PluginContext {
  return {
    ...context,
    // TODO error/warning
  }
}
