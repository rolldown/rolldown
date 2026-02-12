import {
  minify as originalMinify,
  minifySync as originalMinifySync,
  collapseSourcemaps,
  type MinifyOptions as OriginalMinifyOptions,
  type MinifyResult,
  type SourceMap,
} from '../binding.cjs';
import { bindingifySourcemap } from '../types/sourcemap';

type MinifyOptions = OriginalMinifyOptions & {
  inputMap?: SourceMap;
};

/**
 * Minify asynchronously.
 *
 * Note: This function can be slower than `minifySync` due to the overhead of spawning a thread.
 *
 * @experimental
 */
async function minify(
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
 * @experimental
 */
function minifySync(
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

export { minify, minifySync };
export type { MinifyOptions, MinifyResult };
