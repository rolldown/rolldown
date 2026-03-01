import type { BindingBundleAnalyzerPluginConfig } from '../binding.cjs';
import { BuiltinPlugin } from './utils';

/**
 * A plugin that analyzes bundle composition and generates detailed reports.
 *
 * The plugin outputs a file containing detailed information about:
 * - All chunks and their relationships
 * - Modules bundled in each chunk
 * - Import dependencies between chunks
 * - Reachable modules from each entry point
 *
 * @example
 * ```js
 * import { bundleAnalyzerPlugin } from 'rolldown/experimental';
 *
 * export default {
 *   plugins: [
 *     bundleAnalyzerPlugin()
 *   ]
 * }
 * ```
 *
 * @example
 * **Custom filename**
 * ```js
 * import { bundleAnalyzerPlugin } from 'rolldown/experimental';
 *
 * export default {
 *   plugins: [
 *     bundleAnalyzerPlugin({
 *       fileName: 'bundle-analysis.json'
 *     })
 *   ]
 * }
 * ```
 *
 * @example
 * **LLM-friendly markdown output**
 * ```js
 * import { chunkVisualizePlugin } from 'rolldown/experimental';
 *
 * export default {
 *   plugins: [
 *     chunkVisualizePlugin({
 *       format: 'md'
 *     })
 *   ]
 * }
 * ```
 */
export function bundleAnalyzerPlugin(config?: BindingBundleAnalyzerPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:bundle-analyzer', config);
}
