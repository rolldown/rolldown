export { dev } from './api/dev';
export { DevEngine } from './api/dev/dev-engine';
export type { DevOptions, DevWatchOptions } from './api/dev/dev-options';
export { freeExternalMemory, scan } from './api/experimental';
export {
  type BindingClientHmrUpdate,
  BindingRebuildStrategy,
  createTokioRuntime,
  isolatedDeclaration,
  type IsolatedDeclarationsOptions,
  type IsolatedDeclarationsResult,
  isolatedDeclarationSync,
  moduleRunnerTransform,
  type NapiResolveOptions as ResolveOptions,
  type ResolveResult,
  ResolverFactory,
} from './binding.cjs';

export { defineParallelPlugin } from './plugin/parallel-plugin';

// Builtin plugin factory
export {
  isolatedDeclarationPlugin,
  viteBuildImportAnalysisPlugin,
  viteDynamicImportVarsPlugin,
  viteImportGlobPlugin,
  viteJsonPlugin,
  viteLoadFallbackPlugin,
  viteModulePreloadPolyfillPlugin,
  viteReactRefreshWrapperPlugin,
  viteReporterPlugin,
  viteResolvePlugin,
  viteWasmFallbackPlugin,
  viteWebWorkerPostPlugin,
} from './builtin-plugin/constructors';

export {
  /**
   * Alias of `viteDynamicImportVarsPlugin`. Note that this plugin is only intended to be used by Vite.
   */
  viteDynamicImportVarsPlugin as dynamicImportVarsPlugin,
  /**
   * Alias of `viteImportGlobPlugin`. Note that this plugin is only intended to be used by Vite.
   */
  viteImportGlobPlugin as importGlobPlugin,
} from './builtin-plugin/constructors';

export { viteAliasPlugin } from './builtin-plugin/alias-plugin';
export { viteTransformPlugin } from './builtin-plugin/transform-plugin';
export { viteManifestPlugin } from './builtin-plugin/vite-manifest-plugin';

// `__volume` and `__fs` only exist in `rolldown-binding.wasi-browser.js`, so we need to use namespace import to prevent static import error.
import * as binding from './binding.cjs';
/**
 * In-memory file system for browser builds.
 *
 * This is a re-export of the {@link https://github.com/streamich/memfs | memfs} package used by the WASI runtime.
 * It allows you to read and write files to a virtual filesystem when using rolldown in browser environments.
 *
 * - `fs`: A Node.js-compatible filesystem API (`IFs` from memfs)
 * - `volume`: The underlying `Volume` instance that stores the filesystem state
 *
 * Returns `undefined` in Node.js builds (only available in browser builds via `@rolldown/browser`).
 *
 * @example
 * ```typescript
 * import { memfs } from 'rolldown/experimental';
 *
 * // Write files to virtual filesystem before bundling
 * memfs?.volume.fromJSON({
 *   '/src/index.js': 'export const foo = 42;',
 *   '/package.json': '{"name": "my-app"}'
 * });
 *
 * // Read files from the virtual filesystem
 * const content = memfs?.fs.readFileSync('/src/index.js', 'utf8');
 * ```
 *
 * @see {@link https://github.com/streamich/memfs} for more information on the memfs API.
 */
export const memfs: { fs: any; volume: any } | undefined = import.meta.browserBuild
  ? // @ts-expect-error - __fs and __volume are only available in browser builds
    { fs: binding.__fs, volume: binding.__volume }
  : undefined;

// #region util exports
import {
  parse as parse_,
  parseSync as parseSync_,
  type ParseResult as ParseResult_,
  type ParserOptions as ParserOptions_,
} from './utils/parse';
/** @deprecated Use from `rolldown/utils` instead. */
export const parse: typeof parse_ = parse_;
/** @deprecated Use from `rolldown/utils` instead. */
export const parseSync: typeof parseSync_ = parseSync_;
/** @deprecated Use from `rolldown/utils` instead. */
export type ParseResult = ParseResult_;
/** @deprecated Use from `rolldown/utils` instead. */
export type ParserOptions = ParserOptions_;

import {
  minify as minify_,
  minifySync as minifySync_,
  type MinifyOptions as MinifyOptions_,
  type MinifyResult as MinifyResult_,
} from './utils/minify';
/** @deprecated Use from `rolldown/utils` instead. */
export const minify: typeof minify_ = minify_;
/** @deprecated Use from `rolldown/utils` instead. */
export const minifySync: typeof minifySync_ = minifySync_;
/** @deprecated Use from `rolldown/utils` instead. */
export type MinifyOptions = MinifyOptions_;
/** @deprecated Use from `rolldown/utils` instead. */
export type MinifyResult = MinifyResult_;

import {
  transform as transform_,
  transformSync as transformSync_,
  TsconfigCache as TsconfigCache_,
  type TransformOptions as TransformOptions_,
  type TransformResult as TransformResult_,
  type TsconfigCompilerOptions as TsconfigCompilerOptions_,
  type TsconfigRawOptions as TsconfigRawOptions_,
} from './utils/transform';
/** @deprecated Use from `rolldown/utils` instead. */
export const transform: typeof transform_ = transform_;
/** @deprecated Use from `rolldown/utils` instead. */
export const transformSync: typeof transformSync_ = transformSync_;
/** @deprecated Use from `rolldown/utils` instead. */
export type TransformOptions = TransformOptions_;
/** @deprecated Use from `rolldown/utils` instead. */
export type TransformResult = TransformResult_;
/** @deprecated Use from `rolldown/utils` instead. */
export const TsconfigCache: typeof TsconfigCache_ = TsconfigCache_;
/** @deprecated Use from `rolldown/utils` instead. */
export type TsconfigRawOptions = TsconfigRawOptions_;
/** @deprecated Use from `rolldown/utils` instead. */
export type TsconfigCompilerOptions = TsconfigCompilerOptions_;

// #endregion
