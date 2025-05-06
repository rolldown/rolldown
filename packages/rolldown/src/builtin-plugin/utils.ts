import {
  type BindingBuiltinPlugin,
  BindingCallableBuiltinPlugin,
} from '../binding';

import { BuiltinPlugin } from './constructors';

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K];
};

export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  let callablePlugin = new BindingCallableBuiltinPlugin(
    bindingifyBuiltInPlugin(plugin),
  );

  const wrappedPlugin: any = {};
  for (const key in callablePlugin) {
    // @ts-expect-error
    wrappedPlugin[key] = function(...args) {
      // @ts-expect-error
      return callablePlugin[key](...args);
    };
  }
  return wrappedPlugin as BuiltinPlugin & BindingCallableBuiltinPluginLike;
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin._options,
  };
}
