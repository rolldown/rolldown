import {
  BindingBuiltinPluginName,
  BindingReplacePluginConfig,
} from '../binding'

import { BuiltinPlugin } from './constructors'

class ReplacePlugin extends BuiltinPlugin {
  constructor(config?: BindingReplacePluginConfig) {
    super(BindingBuiltinPluginName.ReplacePlugin, config)
  }
}

/**
 * ## Usage
 *
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *    __buildDate__: () => JSON.stringify(new Date()),
 *    __buildVersion: 15
 * })
 * ```
 *
 * ### With options
 *
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *   __buildDate__: () => JSON.stringify(new Date()),
 *   __buildVersion: 15
 * }, {
 *   preventAssignment: false,
 * })
 * ```
 *
 */
export function replacePlugin(
  values: BindingReplacePluginConfig['values'] = {},
  options: Omit<BindingReplacePluginConfig, 'values'> = {},
) {
  return new ReplacePlugin({ ...options, values })
}
