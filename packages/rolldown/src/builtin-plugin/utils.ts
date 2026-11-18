import {
  type BindingBuiltinPlugin,
  type BindingBuiltinPluginName,
  BindingCallableBuiltinPlugin,
  type BindingViteManifestPluginConfig,
} from '../binding.cjs';
import { error, logPluginError } from '../log/logs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import type { ViteManifestPluginConfig } from './vite-manifest-plugin';

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K];
};

// eslint-disable @typescript-eslint/no-unsafe-declaration-merging
export class BuiltinPlugin {
  /** Vite-specific option to control plugin ordering */
  enforce?: 'pre' | 'post';

  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {}
}

type CallableBuiltinPluginContext = {
  plugin: BuiltinPlugin;
  native: BindingCallableBuiltinPlugin;
  callbacks: Function[];
};

// Ownership of the option callbacks. See https://github.com/rolldown/rolldown/issues/10887.
//
// The native plugin keeps each option callback in a ThreadsafeFunction. A ThreadsafeFunction
// is a GC root. If it held the user's callback, the callback would keep its owner alive until
// the native plugin is released. So the native plugin gets a forwarder that holds only a
// `WeakRef` to the callback. The real callbacks stay in `callbacks`. The JS plugin owns
// `callbacks` through the hooks that are bound to the context.
//
// These three parts are necessary for the fix. Do not remove them:
// - `callbacks` is the only strong reference that has the lifetime of the plugin. Without it,
//   the forwarders fail when the caller replaces or deletes `plugin._options`.
// - The Proxy target is `{}`, and only the `get` trap is set. napi-rs reads each option field
//   with `napi_get_named_property`, so the `get` trap is sufficient. An empty target also
//   prevents the invariant checks of a frozen `options` object. The Proxy wraps only
//   top-level function options. A callback in a nested object goes to the native plugin as a
//   strong reference and leaks again.
// - `invokeBuiltinHook` must use `return await`. The suspended frame keeps the context alive
//   while the native hook runs, and the context keeps the callbacks alive. So a forwarder does
//   not find an empty `WeakRef` during a hook call.
export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  const callbacks: Function[] = [];
  const binding = bindingifyBuiltInPlugin(plugin);
  const options = binding.options;
  if (options !== null && (typeof options === 'object' || typeof options === 'function')) {
    binding.options = new Proxy(
      {},
      {
        get(_target, key) {
          const value = Reflect.get(options, key);
          if (typeof value !== 'function') return value;
          callbacks.push(value);
          return invokeWeakCallback.bind(
            undefined,
            new WeakRef(value),
            `${plugin.name}.${String(key)}`,
          );
        },
      },
    );
  }
  const context: CallableBuiltinPluginContext = {
    plugin,
    native: new BindingCallableBuiltinPlugin(binding),
    callbacks,
  };

  const wrappedPlugin: Partial<BindingCallableBuiltinPluginLike> & BuiltinPlugin = plugin;
  for (const key in context.native) {
    const wrappedHook = invokeBuiltinHook.bind(context, key);
    const order = context.native.getOrder(key);
    if (order == undefined) {
      // @ts-expect-error
      wrappedPlugin[key] = wrappedHook;
    } else {
      // @ts-expect-error
      wrappedPlugin[key] = { handler: wrappedHook, order };
    }
  }
  return wrappedPlugin as BuiltinPlugin & BindingCallableBuiltinPluginLike;
}

async function invokeBuiltinHook(this: CallableBuiltinPluginContext, key: string, ...args: any[]) {
  try {
    // @ts-expect-error
    return await this.native[key](...args);
  } catch (e: any) {
    if (e instanceof Error && !/^\s+at /m.test(e.stack ?? '')) {
      // An error from the native side has no stack frames. Capture the frames
      // here. Pass this function so that V8 removes the frames of this wrapper.
      // Do not pass the hook on the plugin: it is a bound function, and V8 does
      // not match a bound function to a frame.
      Error.captureStackTrace(e, invokeBuiltinHook);
    }
    return error(
      logPluginError(e, this.plugin.name, {
        hook: key,
        // `transform(code, id, options)` — the module id is the second argument.
        id: key === 'transform' ? args[1] : undefined,
      }),
    );
  }
}

function invokeWeakCallback(reference: WeakRef<Function>, name: string, ...args: unknown[]) {
  const callback = reference.deref();
  if (!callback) {
    // This path is not possible during a hook call. See the ownership note on
    // `makeBuiltinPluginCallable`.
    throw new Error(`The callback for ${name} was released`);
  }
  return Reflect.apply(callback, undefined, args);
}

export function bindingifyBuiltInPlugin(plugin: BuiltinPlugin): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin._options,
  };
}

export function bindingifyManifestPlugin(
  plugin: BuiltinPlugin,
  pluginContextData: PluginContextData,
): BindingBuiltinPlugin {
  const { isOutputOptionsForLegacyChunks, ...options } =
    plugin._options as ViteManifestPluginConfig;
  return {
    __name: plugin.name,
    options: {
      ...options,
      isLegacy: isOutputOptionsForLegacyChunks
        ? (opts) => {
            return isOutputOptionsForLegacyChunks(pluginContextData.getOutputOptions(opts));
          }
        : undefined,
    } as BindingViteManifestPluginConfig,
  };
}
