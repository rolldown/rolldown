import type {
  MinifyOptions as BindingMinifyOptions,
  PreRenderedChunk,
} from '../binding.cjs';
import type { RolldownOutputPluginOption } from '../plugin';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
import type { ModuleInfo } from '../types/module-info';
import type { RenderedChunk } from '../types/rolldown-output';
import type { NullValue, StringOrRegExp } from '../types/utils';

export type GeneratedCodePreset = 'es5' | 'es2015';

export interface GeneratedCodeOptions {
  /**
   * Whether to use Symbol.toStringTag for namespace objects.
   * @default false
   */
  symbols?: boolean;
  /**
   * Allows choosing one of the presets listed above while overriding some options.
   *
   * ```js
   * export default {
   *   output: {
   *     generatedCode: {
   *       preset: 'es2015',
   *       symbols: false
   *     }
   *   }
   * };
   * ```
   */
  preset?: GeneratedCodePreset;
  /**
   * Whether to add readable names to internal variables for profiling purposes.
   *
   * When enabled, generated code will use descriptive variable names that correspond
   * to the original module names, making it easier to profile and debug the bundled code.
   *
   * @default true when minification is disabled, false when minification is enabled
   */
  profilerNames?: boolean;
}

export type ModuleFormat =
  | 'es'
  | 'cjs'
  | 'esm'
  | 'module'
  | 'commonjs'
  | 'iife'
  | 'umd';

export type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>;

export type ChunkFileNamesFunction = (chunkInfo: PreRenderedChunk) => string;

export type SanitizeFileNameFunction = (name: string) => string;

export interface PreRenderedAsset {
  type: 'asset';
  name?: string;
  names: string[];
  originalFileName?: string;
  originalFileNames: string[];
  source: string | Uint8Array;
}

export type AssetFileNamesFunction = (chunkInfo: PreRenderedAsset) => string;

export type PathsFunction = (id: string) => string;

export type ManualChunksFunction = (
  moduleId: string,
  meta: { getModuleInfo: (moduleId: string) => ModuleInfo | null },
) => string | NullValue;

export type GlobalsFunction = (name: string) => string;

export type AdvancedChunksNameFunction = (
  moduleId: string,
  ctx: ChunkingContext,
) => string | NullValue;

export type AdvancedChunksTestFunction = (
  id: string,
) => boolean | undefined | void;

export type MinifyOptions = Omit<BindingMinifyOptions, 'module' | 'sourcemap'>;

export interface ChunkingContext {
  getModuleInfo(moduleId: string): ModuleInfo | null;
}

export interface OutputOptions {
  dir?: string;
  file?: string;
  exports?: 'auto' | 'named' | 'default' | 'none';
  hashCharacters?: 'base64' | 'base36' | 'hex';
  /**
   * Expected format of generated code.
   * - `'es'`, `'esm'` and `'module'` are the same format, all stand for ES module.
   * - `'cjs'` and `'commonjs'` are the same format, all stand for CommonJS module.
   * - `'iife'` stands for [Immediately Invoked Function Expression](https://developer.mozilla.org/en-US/docs/Glossary/IIFE).
   * - `'umd'` stands for [Universal Module Definition](https://github.com/umdjs/umd).
   *
   * @default 'esm'
   */
  format?: ModuleFormat;
  sourcemap?: boolean | 'inline' | 'hidden';
  sourcemapBaseUrl?: string;
  sourcemapDebugIds?: boolean;
  /**
   * Control which source files are included in the sourcemap ignore list.
   * Files in the ignore list are excluded from debugger stepping and error stack traces.
   *
   * - `false`: Include all source files in the ignore list
   * - `true`: Include no source files in the ignore list
   * - `string`: Files containing this string in their path will be included in the ignore list
   * - `RegExp`: Files matching this regular expression will be included in the ignore list
   * - `function`: Custom function `(source: string, sourcemapPath: string) => boolean` to determine if a source should be ignored
   *
   * :::tip Performance
   * Using static values (`boolean`, `string`, or `RegExp`) is significantly more performant than functions.
   * Calling JavaScript functions from Rust has extremely high overhead, so prefer static patterns when possible.
   * :::
   *
   * ## Examples
   * ```js
   * // ✅ Preferred: Use RegExp for better performance
   * sourcemapIgnoreList: /node_modules/
   *
   * // ✅ Preferred: Use string pattern for better performance
   * sourcemapIgnoreList: "vendor"
   *
   * // ! Use sparingly: Function calls have high overhead
   * sourcemapIgnoreList: (source, sourcemapPath) => {
   *   return source.includes('node_modules') || source.includes('.min.');
   * }
   * ```
   *
   * **default**: /node_modules/
   */
  sourcemapIgnoreList?: boolean | SourcemapIgnoreListOption | StringOrRegExp;
  sourcemapPathTransform?: SourcemapPathTransformOption;
  banner?: string | AddonFunction;
  footer?: string | AddonFunction;
  intro?: string | AddonFunction;
  outro?: string | AddonFunction;
  extend?: boolean;
  esModule?: boolean | 'if-default-prop';
  assetFileNames?: string | AssetFileNamesFunction;
  entryFileNames?: string | ChunkFileNamesFunction;
  chunkFileNames?: string | ChunkFileNamesFunction;
  cssEntryFileNames?: string | ChunkFileNamesFunction;
  cssChunkFileNames?: string | ChunkFileNamesFunction;
  sanitizeFileName?: boolean | SanitizeFileNameFunction;
  /**
   * Control code minification.
   *
   * - `true`: Enable full minification including code compression and dead code elimination
   * - `false`: Disable minification (default)
   * - `'dce-only'`: Only perform dead code elimination without code compression
   * - `MinifyOptions`: Fine-grained control over minification settings
   *
   * @default false
   */
  minify?: boolean | 'dce-only' | MinifyOptions;
  name?: string;
  globals?: Record<string, string> | GlobalsFunction;
  /**
   * Maps external module IDs to paths.
   *
   * Allows customizing the path used when importing external dependencies.
   * This is particularly useful for loading dependencies from CDNs or custom locations.
   *
   * - Object form: Maps module IDs to their replacement paths
   * - Function form: Takes a module ID and returns its replacement path
   *
   * @example
   * ```js
   * {
   *   paths: {
   *     'd3': 'https://cdn.jsdelivr.net/npm/d3@7'
   *   }
   * }
   * ```
   *
   * @example
   * ```js
   * {
   *   paths: (id) => {
   *     if (id.startsWith('lodash')) {
   *       return `https://cdn.jsdelivr.net/npm/${id}`
   *     }
   *     return id
   *   }
   * }
   * ```
   */
  paths?: Record<string, string> | PathsFunction;
  generatedCode?: Partial<GeneratedCodeOptions>;
  externalLiveBindings?: boolean;
  inlineDynamicImports?: boolean;
  /**
   * - Type: `((moduleId: string, meta: { getModuleInfo: (moduleId: string) => ModuleInfo | null }) => string | NullValue)`
   * - Object form is not supported.
   *
   * :::warning
   * - This option is deprecated. Please use `advancedChunks` instead.
   * - If `manualChunks` and `advancedChunks` are both specified, `manualChunks` option will be ignored.
   * :::
   *
   * You could use this option for migration purpose. Under the hood,
   *
   * ```js
   * {
   *   manualChunks: (moduleId, meta) => {
   *     if (moduleId.includes('node_modules')) {
   *       return 'vendor';
   *     }
   *     return null;
   *   }
   * }
   * ```
   *
   * will be transformed to
   *
   * ```js
   * {
   *   advancedChunks: {
   *     groups: [
   *       {
   *         name(moduleId) {
   *           if (moduleId.includes('node_modules')) {
   *             return 'vendor';
   *           }
   *           return null;
   *         },
   *       },
   *     ],
   *   }
   * }
   *
   * ```
   *
   * @deprecated Please use `advancedChunks` instead.
   */
  manualChunks?: ManualChunksFunction;
  /**
   * Allows you to do manual chunking. For deeper understanding, please refer to the in-depth [documentation](https://rolldown.rs/in-depth/advanced-chunks).
   */
  advancedChunks?: {
    /**
     * - Type: `boolean`
     * - Default: `true`
     *
     * By default, each group will also include captured modules' dependencies. This reduces the chance of generating circular chunks.
     *
     * If you want to disable this behavior, it's recommended to both set
     * - `preserveEntrySignatures: false | 'allow-extension'`
     * - `strictExecutionOrder: true`
     *
     * to avoid generating invalid chunks.
     */
    includeDependenciesRecursively?: boolean;
    /**
     * - Type: `number`
     *
     * Global fallback of [`{group}.minSize`](#advancedchunks-groups-minsize), if it's not specified in the group.
     */
    minSize?: number;
    /**
     * - Type: `number`
     *
     * Global fallback of [`{group}.maxSize`](#advancedchunks-groups-maxsize), if it's not specified in the group.
     */
    maxSize?: number;
    /**
     * - Type: `number`
     *
     * Global fallback of [`{group}.maxModuleSize`](#advancedchunks-groups-maxmodulesize), if it's not specified in the group.
     */
    maxModuleSize?: number;
    /**
     * - Type: `number`
     *
     * Global fallback of [`{group}.minModuleSize`](#advancedchunks-groups-minmodulesize), if it's not specified in the group.
     */
    minModuleSize?: number;
    /**
     * - Type: `number`
     *
     * Global fallback of [`{group}.minShareCount`](#advancedchunks-groups-minsharecount), if it's not specified in the group.
     */
    minShareCount?: number;
    /**
     * Groups to be used for advanced chunking.
     */
    groups?: {
      /**
       * - Type: `string | ((moduleId: string, ctx: { getModuleInfo: (moduleId: string) => ModuleInfo | null }) => string | NullValue)`
       *
       * Name of the group. It will be also used as the name of the chunk and replaced the `[name]` placeholder in the `chunkFileNames` option.
       *
       * For example,
       *
       * ```js
       * import { defineConfig } from 'rolldown';
       *
       * export default defineConfig({
       *   advancedChunks: {
       *     groups: [
       *       {
       *         name: 'libs',
       *         test: /node_modules/,
       *       },
       *     ],
       *   },
       * });
       * ```
       * will create a chunk named `libs-[hash].js` in the end.
       *
       * It's ok to have the same name for different groups. Rolldown will deduplicate the chunk names if necessary.
       *
       * # Dynamic `name()`
       *
       * If `name` is a function, it will be called with the module id as the argument. The function should return a string or `null`. If it returns `null`, the module will be ignored by this group.
       *
       * Notice, each returned new name will be treated as a separate group.
       *
       * For example,
       *
       * ```js
       * import { defineConfig } from 'rolldown';
       *
       * export default defineConfig({
       *   advancedChunks: {
       *     groups: [
       *       {
       *         name: (moduleId) => moduleId.includes('node_modules') ? 'libs' : 'app',
       *         minSize: 100 * 1024,
       *       },
       *     ],
       *   },
       * });
       * ```
       *
       * :::warning
       * Constraints like `minSize`, `maxSize`, etc. are applied separately for different names returned by the function.
       * :::
       */
      name:
        | string
        | AdvancedChunksNameFunction;
      /**
       * - Type: `string | RegExp | ((id: string) => boolean | undefined | void);`
       *
       * Controls which modules are captured in this group.
       *
       * - If `test` is a string, the module whose id contains the string will be captured.
       * - If `test` is a regular expression, the module whose id matches the regular expression will be captured.
       * - If `test` is a function, modules for which `test(id)` returns `true` will be captured.
       * - If `test` is empty, any module will be considered as matched.
       *
       * :::warning
       * When using regular expression, it's recommended to use `[\\/]` to match the path separator instead of `/` to avoid potential issues on Windows.
       * - ✅ Recommended: `/node_modules[\\/]react/`
       * - ❌ Not recommended: `/node_modules/react/`
       * :::
       */
      test?: StringOrRegExp | AdvancedChunksTestFunction;
      /**
       * - Type: `number`
       * - Default: `0`
       *
       * Priority of the group. Group with higher priority will be chosen first to match modules and create chunks. When converting the group to a chunk, modules of that group will be removed from other groups.
       *
       * If two groups have the same priority, the group whose index is smaller will be chosen.
       *
       * For example,
       *
       * ```js
       * import { defineConfig } from 'rolldown';
       *
       * export default defineConfig({
       *  advancedChunks: {
       *   groups: [
       *      {
       *        name: 'react',
       *        test: /node_modules[\\/]react/,
       *        priority: 1,
       *      },
       *      {
       *        name: 'other-libs',
       *        test: /node_modules/,
       *        priority: 2,
       *      },
       *   ],
       * });
       * ```
       *
       * This is a clearly __incorrect__ example. Though `react` group is defined before `other-libs`, it has a lower priority, so the modules in `react` group will be captured in `other-libs` group.
       */
      priority?: number;
      /**
       * - Type: `number`
       * - Default: `0`
       *
       * Minimum size in bytes of the desired chunk. If the accumulated size of the captured modules by this group is smaller than this value, it will be ignored. Modules in this group will fall back to the `automatic chunking` if they are not captured by any other group.
       */
      minSize?: number;
      /**
       * - Type: `number`
       * - Default: `1`
       *
       * Controls if a module should be captured based on how many entry chunks reference it.
       */
      minShareCount?: number;
      /**
       * - Type: `number`
       * - Default: `Infinity`
       *
       * If the accumulated size in bytes of the captured modules by this group is larger than this value, this group will be split into multiple groups that each has size close to this value.
       */
      maxSize?: number;
      /**
       * - Type: `number`
       * - Default: `Infinity`
       *
       * Controls a module could only be captured if its size in bytes is smaller or equal than this value.
       */
      maxModuleSize?: number;
      /**
       * - Type: `number`
       * - Default: `0`
       *
       * Controls a module could only be captured if its size in bytes is larger or equal than this value.
       */
      minModuleSize?: number;
    }[];
  };
  /**
   * Control comments in the output.
   *
   * - `none`: no comments
   * - `inline`: preserve comments that contain `@license`, `@preserve` or starts with `//!` `/*!`
   */
  legalComments?: 'none' | 'inline';
  plugins?: RolldownOutputPluginOption;
  polyfillRequire?: boolean;
  hoistTransitiveImports?: false;
  preserveModules?: boolean;
  virtualDirname?: string;
  preserveModulesRoot?: string;
  topLevelVar?: boolean;
  /**
   * - Type: `boolean`
   * - Default: `true` for format `es` or if `output.minify` is `true` or object, `false` otherwise
   *
   * Whether to minify internal exports.
   */
  minifyInternalExports?: boolean;
  /**
   * - Type: `boolean`
   * - Default: `false`
   *
   * Clean output directory before emitting output.
   */
  cleanDir?: boolean;
  /** Keep function and class names after bundling.
   *
   * When enabled, the bundler will preserve the original names of functions and classes
   * in the output, which is useful for debugging and error stack traces.
   *
   * @default false
   */
  keepNames?: boolean;
}

interface OverwriteOutputOptionsForCli {
  banner?: string;
  footer?: string;
  intro?: string;
  outro?: string;
  esModule?: boolean;
  globals?: Record<string, string>;
  advancedChunks?: {
    minSize?: number;
    minShareCount?: number;
  };
}

export type OutputCliOptions =
  & Omit<
    OutputOptions,
    | keyof OverwriteOutputOptionsForCli
    | 'sourcemapIgnoreList'
    | 'sourcemapPathTransform'
  >
  & OverwriteOutputOptionsForCli;
