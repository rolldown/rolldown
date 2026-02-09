import type { BindingChunkVisualizePluginConfig } from '../binding.cjs';
import { BuiltinPlugin } from './utils';

/**
 * A plugin that generates chunk visualization data for analyzing bundle composition.
 *
 * The plugin outputs a JSON file containing detailed information about:
 * - All chunks and their relationships
 * - Modules bundled in each chunk
 * - Import dependencies between chunks
 * - Reachable modules from each entry point
 *
 * @example
 * ```js
 * import { chunkVisualizePlugin } from 'rolldown/experimental';
 *
 * export default {
 *   plugins: [
 *     chunkVisualizePlugin()
 *   ]
 * }
 * ```
 *
 * @example
 * **Custom filename**
 * ```js
 * import { chunkVisualizePlugin } from 'rolldown/experimental';
 *
 * export default {
 *   plugins: [
 *     chunkVisualizePlugin({
 *       fileName: 'bundle-analysis.json'
 *     })
 *   ]
 * }
 * ```
 */
export function chunkVisualizePlugin(config?: BindingChunkVisualizePluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:chunk-visualize', config);
}
