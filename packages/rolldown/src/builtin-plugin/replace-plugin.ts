import type { BindingReplacePluginConfig } from '../binding';

import { BuiltinPlugin } from './utils';

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
  return BuiltinPlugin.getInstance('builtin:replace', { ...options, values });
}
