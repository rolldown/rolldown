import {
  BindingBuiltinPlugin,
  BindingCallableBuiltinPlugin,
  isCallableCompatibleBuiltinPlugin as isCallableCompatibleBuiltinPluginInternal,
} from '../binding'

import { BuiltinPlugin } from './constructors'

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K]
}

export function makeBuiltinPluginCallable(plugin: BuiltinPlugin) {
  let callablePlugin = new BindingCallableBuiltinPlugin(
    bindingifyBuiltInPlugin(plugin),
  )

  const wrappedPlugin: Partial<BindingCallableBuiltinPluginLike> & {
    _original: BindingCallableBuiltinPlugin
  } = {
    _original: callablePlugin,
  }
  for (const key in callablePlugin) {
    if (key === 'name') {
      wrappedPlugin[key] = callablePlugin[key]
    } else {
      // @ts-expect-error
      wrappedPlugin[key] = function (...args) {
        // @ts-expect-error
        return callablePlugin[key](...args)
      }
    }
  }
  return wrappedPlugin as BindingCallableBuiltinPluginLike & {
    _original: BindingCallableBuiltinPlugin
  }
}

export function isCallableBuiltinPlugin(plugin: any): boolean {
  return (
    '_original' in plugin &&
    plugin._original instanceof BindingCallableBuiltinPlugin
  )
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin.options,
  }
}

export function isCallableCompatibleBuiltinPlugin(
  plugin: any,
): plugin is BuiltinPlugin {
  return (
    plugin instanceof BuiltinPlugin &&
    isCallableCompatibleBuiltinPluginInternal(bindingifyBuiltInPlugin(plugin))
  )
}
