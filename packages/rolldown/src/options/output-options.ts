import type { BindingMinifyOptions, PreRenderedChunk } from '../binding';
import type { RolldownOutputPluginOption } from '../plugin';
import type { ChunkingContext } from '../types/chunking-context';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
import type { RenderedChunk } from '../types/rolldown-output';
import type { NullValue, StringOrRegExp } from '../types/utils';

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

export interface PreRenderedAsset {
  names: string[];
  originalFileNames: string[];
  source: string | Uint8Array;
  type: 'asset';
}

export type AssetFileNamesFunction = (chunkInfo: PreRenderedAsset) => string;

export type GlobalsFunction = (name: string) => string;

export type MinifyOptions = BindingMinifyOptions;

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
  sourcemapDebugIds?: boolean;
  sourcemapIgnoreList?: boolean | SourcemapIgnoreListOption;
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
  sanitizeFileName?: boolean | ((name: string) => string);
  minify?: boolean | 'dce-only' | MinifyOptions;
  name?: string;
  globals?: Record<string, string> | GlobalsFunction;
  externalLiveBindings?: boolean;
  inlineDynamicImports?: boolean;
  /**
   * - Type: `((moduleId: string) => string | NullValue)`
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
  manualChunks?: (moduleId: string, meta: {}) => string | NullValue;
  /**
   * Allows you to do manual chunking. For deeper understanding, please refer to the in-depth [documentation](https://rolldown.rs/guide/in-depth/advanced-chunks).
   */
  advancedChunks?: {
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
        | ((
          moduleId: string,
          ctx: ChunkingContext,
        ) => string | NullValue);
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
      test?: StringOrRegExp | ((id: string) => boolean | undefined | void);
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
