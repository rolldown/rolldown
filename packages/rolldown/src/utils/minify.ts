import {
  minify as originalMinify,
  minifySync as originalMinifySync,
  collapseSourcemaps,
  type MinifyOptions as OriginalMinifyOptions,
  type MinifyResult as OriginalMinifyResult,
  type SourceMap,
} from '../binding.cjs';
import { bindingifySourcemap } from '../types/sourcemap';

/**
 * Options for minification.
 *
 * @category Utilities
 */
export interface MinifyOptions extends OriginalMinifyOptions {
  inputMap?: SourceMap;
}

/**
 * The result of minification.
 *
 * @category Utilities
 */
export interface MinifyResult extends OriginalMinifyResult {}

/**
 * Minify asynchronously.
 *
 * Note: This function can be slower than {@linkcode minifySync} due to the overhead of spawning a thread.
 *
 * @category Utilities
 * @experimental
 */
export async function minify(
  filename: string,
  sourceText: string,
  options?: MinifyOptions | null,
): Promise<MinifyResult> {
  const inputMap = bindingifySourcemap(options?.inputMap);
  const result = await originalMinify(filename, sourceText, options);
  if (result.map && inputMap) {
    result.map = {
      version: 3,
      ...collapseSourcemaps([inputMap, bindingifySourcemap(result.map)!]),
    } as SourceMap;
  }
  return result;
}

/**
 * Minify synchronously.
 *
 * @category Utilities
 * @experimental
 */
export function minifySync(
  filename: string,
  sourceText: string,
  options?: MinifyOptions | null,
): MinifyResult {
  const inputMap = bindingifySourcemap(options?.inputMap);
  const result = originalMinifySync(filename, sourceText, options);
  if (result.map && inputMap) {
    result.map = {
      version: 3,
      ...collapseSourcemaps([inputMap, bindingifySourcemap(result.map)!]),
    } as SourceMap;
  }
  return result;
}
