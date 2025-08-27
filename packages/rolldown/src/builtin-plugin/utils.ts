import {
  type BindingBuiltinPlugin,
  type BindingBuiltinPluginName,
  BindingCallableBuiltinPlugin,
} from '../binding';
import { error, logPluginError } from '../log/logs';

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K];
};

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {}
}
const CLASS_KEY = Symbol.for('BuiltinPlugin');

// @ts-ignore
if (!globalThis[CLASS_KEY]) {
  // @ts-ignore
  globalThis[CLASS_KEY] = BuiltinPlugin;
}

// We need to disable global class cache in some scenarios
// - When we run tests, we need to ensure the globalThis is not polluted between tests or the test will fail(Since the test)
// - User run rolldown in different worker that may pollute the globalThis just like test scenario
const disableGlobalClassCache = process.env.ROLLDOWN_TEST === '1' ||
  process.env.DISABLE_GLOBAL_CLASS_CACHE === '1';

// Our cli use `cli.mjs` as entry, but the configuration maybe use `cli.cjs` entry(use `require` to load builtin plugins or use `cts` configuration).
// Use this pattern to ensure the whole process use same `BuiltinPlugin` class.
// See https://github.com/rolldown/rolldown/blob/1c4a37c1e98f44b0fe5700be5df9e7a871d9df05/packages/rolldown/src/utils/normalize-plugin-option.ts?plain=1#L44.
export function createBuiltinPlugin(
  name: BindingBuiltinPluginName,
  options?: unknown,
): BuiltinPlugin {
  if (disableGlobalClassCache) {
    return new BuiltinPlugin(name, options);
  }
  // @ts-ignore
  const Cls = globalThis[CLASS_KEY] as typeof BuiltinPlugin;
  return new Cls(name, options);
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
