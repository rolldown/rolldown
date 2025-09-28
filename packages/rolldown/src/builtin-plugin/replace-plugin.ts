import type { BindingReplacePluginConfig } from '../binding';
import { logger } from '../cli/logger';
import { BuiltinPlugin } from './constructor';

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
  let hasNonStringValues = false;

  // Convert all values to string during runtime
  Object.keys(values).forEach(key => {
    const value = values[key];
    if (typeof value !== 'string') {
      hasNonStringValues = true;
      values[key] = String(value);
    }
  });

  if (hasNonStringValues) {
    logger.warn(
      'Some values provided to `replacePlugin` are not strings. They will be converted to strings, but for better performance consider converting them manually.',
    );
  }

  return new BuiltinPlugin('builtin:replace', { ...options, values });
}
