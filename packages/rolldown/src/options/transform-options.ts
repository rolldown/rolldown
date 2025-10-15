import type { TransformOptions as OxcTransformOptions } from '../binding';

export interface TransformOptions extends
  Omit<
    OxcTransformOptions,
    | 'sourceType'
    | 'lang'
    | 'cwd'
    | 'sourcemap'
    | 'define'
    | 'inject'
    | 'jsx'
  >
{
  /**
   * Replace global variables or [property accessors](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Property_accessors) with the provided values.
   *
   * # Examples
   *
   * - Replace the global variable `IS_PROD` with `true`
   *
   * ```js rolldown.config.js
   * export default defineConfig({ transform: { define: { IS_PROD: 'true' } } })
   * ```
   *
   * Result:
   *
   * ```js
   * // Input
   * if (IS_PROD) {
   *   console.log('Production mode')
   * }
   *
   * // After bundling
   * if (true) {
   *   console.log('Production mode')
   * }
   * ```
   *
   * - Replace the property accessor `process.env.NODE_ENV` with `'production'`
   *
   * ```js rolldown.config.js
   * export default defineConfig({ transform: { define: { 'process.env.NODE_ENV': "'production'" } } })
   * ```
   *
   * Result:
   *
   * ```js
   * // Input
   * if (process.env.NODE_ENV === 'production') {
   *  console.log('Production mode')
   * }
   *
   * // After bundling
   * if ('production' === 'production') {
   * console.log('Production mode')
   * }
   *
   * ```
   */
  define?: Record<string, string>;
  /**
   * Inject import statements on demand.
   *
   * The API is aligned with `@rollup/plugin-inject`.
   *
   * ## Supported patterns
   * ```js
   * {
   *   // import { Promise } from 'es6-promise'
   *   Promise: ['es6-promise', 'Promise'],
   *
   *   // import { Promise as P } from 'es6-promise'
   *   P: ['es6-promise', 'Promise'],
   *
   *   // import $ from 'jquery'
   *   $: 'jquery',
   *
   *   // import * as fs from 'node:fs'
   *   fs: ['node:fs', '*'],
   *
   *   // Inject shims for property access pattern
   *   'Object.assign': path.resolve( 'src/helpers/object-assign.js' ),
   * }
   * ```
   */
  inject?: Record<string, string | [string, string]>;
  /**
   * Remove labeled statements with these label names.
   *
   * Labeled statements are JavaScript statements prefixed with a label identifier.
   * This option allows you to strip specific labeled statements from the output,
   * which is useful for removing debug-only code in production builds.
   *
   * ## Example
   *
   * ```js rolldown.config.js
   * export default defineConfig({ transform: { dropLabels: ['DEBUG', 'DEV'] } })
   * ```
   *
   * Result:
   *
   * ```js
   * // Input
   * DEBUG: console.log('Debug info');
   * DEV: {
   *   console.log('Development mode');
   * }
   * console.log('Production code');
   *
   * // After bundling
   * console.log('Production code');
   * ```
   */
  dropLabels?: string[];
  jsx?: OxcTransformOptions['jsx'];
}
