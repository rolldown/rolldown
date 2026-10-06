import type { MaybePromise, NullValue } from '../types/utils';
import type { ChangeEvent, ObjectHook, Plugin } from './index';
import type { PluginContext } from './plugin-context';

// Plugin hooks the engine supports but that are not part of the public plugin
// API: nothing here is exported from any package entry point, so these types
// never reach the published `.d.ts` or the docs. Today all of them are dev-only
// hooks consumed by Vite's bundled-dev adapter, which attaches them to plugin
// objects at runtime (it declares the shapes structurally on its side instead
// of importing them). Exposing one publicly again means moving it back to
// `FunctionPluginHooks` in `./index`.

/**
 * Names of the hidden hooks, alongside {@linkcode InternalFunctionPluginHooks}.
 * Kept separate from `ENUMERATED_INPUT_PLUGIN_HOOK_NAMES` so the names stay out
 * of the published `.d.ts` (that list's type is reachable from public types).
 */
export const INTERNAL_PLUGIN_HOOK_NAMES = ['hotUpdate'] as const;

interface HotUpdateOptions {
  type: ChangeEvent;
  /** Normalized absolute path of the changed file. */
  file: string;
  /** The affected module ids as currently computed (raw module ids). */
  modules: string[];
}

interface InternalFunctionPluginHooks {
  /**
   * Dev mode only. Runs once per changed file during an HMR update, after
   * Rolldown maps the file to its default affected modules and before those
   * modules are re-fetched.
   *
   * Return an array of module ids to replace the affected set for the plugins
   * after this one (an empty array suppresses the update for this file); return
   * nothing to pass the current set through unchanged. Module ids are raw ids
   * and are validated against the module graph — unknown ids are dropped. Use
   * `this.getModuleInfo()` to inspect a module's importers.
   *
   * @kind async sequential
   */
  hotUpdate: (this: PluginContext, options: HotUpdateOptions) => MaybePromise<string[] | NullValue>;
}

type InternalPluginHooks = {
  [K in keyof InternalFunctionPluginHooks]: ObjectHook<InternalFunctionPluginHooks[K]>;
};

export interface PluginWithInternalHooks extends Plugin, Partial<InternalPluginHooks> {}
