import type { BindingReplacePluginConfig } from '../binding';

import { BuiltinPlugin, createBuiltinPlugin } from './utils';

/**
 * Replaces targeted strings in files while bundling.
 *
 * @example
 * // Basic usage
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *    __buildVersion: 15
 * })
 * ```
 * @example
 * // With options
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *   __buildVersion: 15
 * }, {
 *   preventAssignment: false,
 * })
 * ```
 */
export function replacePlugin(
  values: BindingReplacePluginConfig['values'] = {},
  options: Omit<BindingReplacePluginConfig, 'values'> = {},
): BuiltinPlugin {
  // Convert all values to string during runtime
  Object.keys(values).forEach(key => {
    values[key] = values[key].toString();
  });

  return createBuiltinPlugin('builtin:replace', { ...options, values });
}
