import {
  type BindingBuiltinPlugin,
  type BindingBuiltinPluginName,
  BindingCallableBuiltinPlugin,
} from '../binding';
import { error, logPluginError } from '../log/logs';

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K];
};

const CLASS_SYMBOL = Symbol.for('BuiltinPlugin');

// Our cli use `.mjs`, but the configuration maybe use `.cjs` entry(if use require to load builtin plugins or use cts configuration).
// Use this pattern to ensure the whole process use same `BuiltinPlugin` class.
export class BuiltinPlugin {
  // Make constructor private to enforce singleton pattern
  //
  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {
  }

  static getInstance(
    name: BindingBuiltinPluginName,
    _options?: unknown,
  ): BuiltinPlugin {
    // @ts-ignore
    if (!globalThis[CLASS_SYMBOL]) {
      // @ts-ignore
      globalThis[CLASS_SYMBOL] = BuiltinPlugin;
    }
    // @ts-ignore
    return new globalThis[CLASS_SYMBOL](name, _options);
  }
}

export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  let callablePlugin = new BindingCallableBuiltinPlugin(
    bindingifyBuiltInPlugin(plugin),
  );

  const wrappedPlugin:
    & Partial<BindingCallableBuiltinPluginLike>
    & BuiltinPlugin = plugin;
  for (const key in callablePlugin) {
    // @ts-expect-error
    wrappedPlugin[key] = async function(...args) {
      try {
        // @ts-expect-error
        return await callablePlugin[key](...args);
      } catch (e: any) {
        if (e instanceof Error && !e.stack?.includes('at ')) {
          Error.captureStackTrace(
            e,
            // @ts-expect-error
            wrappedPlugin[key],
          );
        }
        return error(
          logPluginError(e, plugin.name, {
            hook: key,
            id: key === 'transform' ? args[2] : undefined,
          }),
        );
      }
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
