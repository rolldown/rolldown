import {
  enhancedTransform as originalTransform,
  enhancedTransformSync as originalTransformSync,
  type BindingEnhancedTransformOptions,
  type BindingEnhancedTransformResult,
  TsconfigCache,
} from '../binding.cjs';
import type { RolldownLog } from '../get-log-filter';
import { normalizeBindingError } from './error';

/**
 * Options for transforming a code.
 *
 * @category Utilities
 */
export interface TransformOptions extends BindingEnhancedTransformOptions {}

/**
 * Result of transforming a code.
 *
 * @category Utilities
 */
export type TransformResult = Omit<BindingEnhancedTransformResult, 'errors' | 'warnings'> & {
  errors: Error[];
  warnings: RolldownLog[];
};

export { TsconfigCache };
export type {
  BindingTsconfigRawOptions as TsconfigRawOptions,
  BindingTsconfigCompilerOptions as TsconfigCompilerOptions,
} from '../binding.cjs';

/**
 * Transpile a JavaScript or TypeScript into a target ECMAScript version, asynchronously.
 *
 * Note: This function can be slower than `transformSync` due to the overhead of spawning a thread.
 *
 * @param filename The name of the file being transformed. If this is a
 * relative path, consider setting the {@linkcode TransformOptions#cwd} option.
 * @param sourceText The source code to transform.
 * @param options The transform options including tsconfig and inputMap. See {@linkcode TransformOptions} for more information.
 * @param cache Optional tsconfig cache for reusing resolved tsconfig across multiple transforms.
 * Only used when `options.tsconfig` is `true`.
 *
 * @returns a promise that resolves to an object containing the transformed code,
 * source maps, and any errors that occurred during parsing or transformation.
 *
 * @category Utilities
 * @experimental
 */
export async function transform(
  filename: string,
  sourceText: string,
  options?: TransformOptions | null,
  cache?: TsconfigCache | null,
): Promise<TransformResult> {
  const result = await originalTransform(filename, sourceText, options, cache);
  return {
    ...result,
    errors: result.errors.map(normalizeBindingError),
    warnings: result.warnings.map((w) => w.field0 as RolldownLog),
  };
}

/**
 * Transpile a JavaScript or TypeScript into a target ECMAScript version.
 *
 * @param filename The name of the file being transformed. If this is a
 * relative path, consider setting the {@linkcode TransformOptions#cwd} option.
 * @param sourceText The source code to transform.
 * @param options The transform options including tsconfig and inputMap. See {@linkcode TransformOptions} for more information.
 * @param cache Optional tsconfig cache for reusing resolved tsconfig across multiple transforms.
 * Only used when `options.tsconfig` is `true`.
 *
 * @returns an object containing the transformed code, source maps, and any errors
 * that occurred during parsing or transformation.
 *
 * @category Utilities
 * @experimental
 */
export function transformSync(
  filename: string,
  sourceText: string,
  options?: TransformOptions | null,
  cache?: TsconfigCache | null,
): TransformResult {
  const result = originalTransformSync(filename, sourceText, options, cache);
  return {
    ...result,
    errors: result.errors.map(normalizeBindingError),
    warnings: result.warnings.map((w) => w.field0 as RolldownLog),
  };
}
