import type { PreRenderedChunk } from '../binding';
import { RolldownOutputPluginOption } from '../plugin';
import {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
import { RenderedChunk } from '../types/rolldown-output';
import type { StringOrRegExp } from '../types/utils';

export type ModuleFormat =
  | 'es'
  | 'cjs'
  | 'esm'
  | 'module'
  | 'commonjs'
  | 'iife'
  | 'umd'
  | 'experimental-app';

export type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>;

export type ChunkFileNamesFunction = (chunkInfo: PreRenderedChunk) => string;

export interface MinifyOptions {
  mangle: boolean;
  compress: boolean;
  removeWhitespace: boolean;
}

export interface PreRenderedAsset {
  names: string[];
  originalFileNames: string[];
  source: string | Uint8Array;
  type: 'asset';
}

export type AssetFileNamesFunction = (chunkInfo: PreRenderedAsset) => string;

export type GlobalsFunction = (name: string) => string;

export type ESTarget =
  | 'es6'
  | 'es2015'
  | 'es2016'
  | 'es2017'
  | 'es2018'
  | 'es2019'
  | 'es2020'
  | 'es2021'
  | 'es2022'
  | 'es2023'
  | 'es2024'
  | 'esnext';

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
   * Allows you to do advanced chunking. Use it to reduce the number of common chunks or split out a chunk that hardly changes to obtain better caching.
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
       * - Type: `string`
       *
       * Name of the group. It will be also used as the name of the chunk and replaced the `[name]` placeholder in the `chunkFileNames` option.
       */
      name: string;
      /**
       * - Type: `string | RegExp`
       *
       * Controls which modules are captured in this group.
       *
       * If `test` is a string, the module whose id contains the string will be captured.
       * If `test` is a regular expression, the module whose id matches the regular expression will be captured.
       * if `test` is empty, any module will be considered as matched.
       */
      test?: StringOrRegExp;
      /**
       * - Type: `number`
       *
       * Priority of the group. Group with higher priority will be chosen first to match modules and create chunks. When converting the group to a chunk, modules of that group will be removed from other groups.
       *
       * If two groups have the same priority, the group whose index is smaller will be chosen.
       */
      priority?: number;
      /**
       * - Type: `number`
       * - Default: `0`
       *
       * Minimum size of the desired chunk. If accumulated size of captured modules is smaller than this value, this group will be ignored.s
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
       * If final size of this group is larger than this value, this group will be spit into multiple groups that each has size closed to this value.
       */
      maxSize?: number;
      /**
       * - Type: `number`
       * - Default: `Infinity`
       *
       * Controls a module could only be captured if its size is smaller or equal than this value.
       */
      maxModuleSize?: number;
      /**
       * - Type: `number`
       * - Default: `0`
       *
       * Controls a module could only be captured if its size is larger or equal than this value.
       */
      minModuleSize?: number;
    }[];
  };
  /**
   * Control comments in the output.
   *
   * - `none`: no comments
   * - `preserve-legal`: preserve comments that contain `@license`, `@preserve` or starts with `//!` `/*!`
   */
  comments?: 'none' | 'preserve-legal';
  plugins?: RolldownOutputPluginOption;
  polyfillRequire?: boolean;
  target?: ESTarget;
  hoistTransitiveImports?: false;
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
