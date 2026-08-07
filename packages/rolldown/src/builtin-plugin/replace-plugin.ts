import type { BindingReplacePluginConfig } from '../binding.cjs';
import { BuiltinPlugin, makeBuiltinPluginCallable } from './utils';

/**
 * A replacement is either a literal, or a function called with the id of the module being
 * transformed. The function runs once per match, like `@rollup/plugin-replace`.
 */
export type ReplacementValue =
  | string
  | number
  | boolean
  | ((id: string) => string | Promise<string>);

/**
 * Replaces targeted strings in files while bundling.
 *
 * @example
 * **Basic usage**
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *    __buildVersion: 15
 * })
 * ```
 * @example
 * **With options**
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *   __buildVersion: 15
 * }, {
 *   preventAssignment: false,
 * })
 * ```
 *
 * @see https://rolldown.rs/builtin-plugins/replace
 * @category Builtin Plugins
 */
export function replacePlugin(
  values: Record<string, ReplacementValue> = {},
  options: Omit<BindingReplacePluginConfig, 'values' | 'valueCallbacks'> = {},
): BuiltinPlugin {
  const stringValues: BindingReplacePluginConfig['values'] = {};
  let valueCallbacks: BindingReplacePluginConfig['valueCallbacks'];

  Object.keys(values).forEach((key) => {
    const value = values[key];
    if (typeof value === 'function') {
      valueCallbacks ??= {};
      valueCallbacks[key] = async (id: string) => String(await value(id));
    } else {
      // Convert all values to string during runtime
      stringValues[key] = typeof value === 'string' ? value : String(value);
    }
  });

  return makeBuiltinPluginCallable(
    new BuiltinPlugin('builtin:replace', { ...options, values: stringValues, valueCallbacks }),
  );
}
