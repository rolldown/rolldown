import type { MinifyOptions as BindingMinifyOptions, PreRenderedChunk } from '../binding.cjs';
import type { RolldownOutputPluginOption } from '../plugin';
import type { SourcemapIgnoreListOption, SourcemapPathTransformOption } from '../types/misc';
import type { ModuleInfo } from '../types/module-info';
import type { RenderedChunk } from '../types/rolldown-output';
import type { NullValue, StringOrRegExp } from '../types/utils';
import type { AssetSource } from '../utils/asset-source';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { InputOptions } from './input-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { InternalModuleFormat } from './normalized-output-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { Plugin } from '../plugin';

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
   *
   * @default 'es2015'
   */
  preset?: GeneratedCodePreset;
  /**
   * Whether to add readable names to internal variables for profiling purposes.
   *
   * When enabled, generated code will use descriptive variable names that correspond
   * to the original module names, making it easier to profile and debug the bundled code.
   *
   * @default false
   *
   * {@include ./docs/output-generated-code-profiler-names.md}
   */
  profilerNames?: boolean;
}

/** @inline */
export type ModuleFormat = 'es' | 'cjs' | 'esm' | 'module' | 'commonjs' | 'iife' | 'umd';

/** @inline */
export type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>;

/** @inline */
export type ChunkFileNamesFunction = (chunkInfo: PreRenderedChunk) => string;

/** @inline */
export type SanitizeFileNameFunction = (name: string) => string;

/** @category Plugin APIs */
export interface PreRenderedAsset {
  type: 'asset';
  /** @deprecated Use {@linkcode names} instead. */
  name?: string;
  names: string[];
  /** @deprecated Use {@linkcode originalFileNames} instead. */
  originalFileName?: string;
  /** The list of the absolute paths to the original file of this asset. */
  originalFileNames: string[];
  /** The content of this asset. */
  source: AssetSource;
}

/** @inline */
export type AssetFileNamesFunction = (chunkInfo: PreRenderedAsset) => string;

/** @inline */
export type PathsFunction = (id: string) => string;

/** @inline */
export type ManualChunksFunction = (
  moduleId: string,
  meta: { getModuleInfo: (moduleId: string) => ModuleInfo | null },
) => string | NullValue;

/** @inline */
export type GlobalsFunction = (name: string) => string;

/** @category Plugin APIs */
export type CodeSplittingNameFunction = (
  moduleId: string,
  ctx: ChunkingContext,
) => string | NullValue;

/** @inline */
export type CodeSplittingTestFunction = (id: string) => boolean | undefined | void;

export type MinifyOptions = Omit<BindingMinifyOptions, 'module' | 'sourcemap'>;

/** @inline */
export interface ChunkingContext {
  getModuleInfo(moduleId: string): ModuleInfo | null;
}

export interface OutputOptions {
  /**
   * The directory in which all generated chunks are placed.
   *
   * The {@linkcode file | output.file} option can be used instead if only a single chunk is generated.
   *
   * {@include ./docs/output-dir.md}
   *
   * @default 'dist'
   */
  dir?: string;
  /**
   * The file path for the single generated chunk.
   *
   * The {@linkcode dir | output.dir} option should be used instead if multiple chunks are generated.
   */
  file?: string;
  /**
   * Which exports mode to use.
   *
   * {@include ./docs/output-exports.md}
   *
   * @default 'auto'
   */
  exports?: 'auto' | 'named' | 'default' | 'none';
  /**
   * Specify the character set that Rolldown is allowed to use in file hashes.
   *
   * - `'base64'`: Uses url-safe base64 characters (0-9, a-z, A-Z, -, _). This will produce the shortest hashes.
   * - `'base36'`: Uses alphanumeric characters (0-9, a-z)
   * - `'hex'`: Uses hexadecimal characters (0-9, a-f)
   *
   * @default 'base64'
   */
  hashCharacters?: 'base64' | 'base36' | 'hex';
  /**
   * Expected format of generated code.
   *
   * - `'es'`, `'esm'` and `'module'` are the same format, all stand for ES module.
   * - `'cjs'` and `'commonjs'` are the same format, all stand for CommonJS module.
   * - `'iife'` stands for [Immediately Invoked Function Expression](https://developer.mozilla.org/en-US/docs/Glossary/IIFE).
   * - `'umd'` stands for [Universal Module Definition](https://github.com/umdjs/umd).
   *
   * @default 'esm'
   *
   * {@include ./docs/output-format.md}
   */
  format?: ModuleFormat;
  /**
   * Whether to generate sourcemaps.
   *
   * - `false`: No sourcemap will be generated.
   * - `true`: A separate sourcemap file will be generated.
   * - `'inline'`: The sourcemap will be appended to the output file as a data URL.
   * - `'hidden'`: A separate sourcemap file will be generated, but the link to the sourcemap (`//# sourceMappingURL` comment) will not be included in the output file.
   *
   * @default false
   */
  sourcemap?: boolean | 'inline' | 'hidden';
  /**
   * The base URL for the links to the sourcemap file in the output file.
   *
   * By default, relative URLs are generated. If this option is set, an absolute URL with that base URL will be generated. This is useful when deploying source maps to a different location than your code, such as a CDN or separate debugging server.
   */
  sourcemapBaseUrl?: string;
  /**
   * Whether to include [debug IDs](https://github.com/tc39/ecma426/blob/main/proposals/debug-id.md) in the sourcemap.
   *
   * When `true`, a unique debug ID will be emitted in source and sourcemaps which streamlines identifying sourcemaps across different builds.
   *
   * @default false
   */
  sourcemapDebugIds?: boolean;
  /**
   * Control which source files are included in the sourcemap ignore list.
   *
   * Files in the ignore list are excluded from debugger stepping and error stack traces.
   *
   * - `false`: Include no source files in the ignore list
   * - `true`: Include all source files in the ignore list
   * - `string`: Files containing this string in their path will be included in the ignore list
   * - `RegExp`: Files matching this regular expression will be included in the ignore list
   * - `function`: Custom function to determine if a source should be ignored
   *
   * :::tip Performance
   * Using static values (`boolean`, `string`, or `RegExp`) is significantly more performant than functions.
   * Calling JavaScript functions from Rust has extremely high overhead, so prefer static patterns when possible.
   * :::
   *
   * @example
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
   * @default /node_modules/
   */
  sourcemapIgnoreList?: boolean | SourcemapIgnoreListOption | StringOrRegExp;
  /**
   * A transformation to apply to each path in a sourcemap.
   *
   * @example
   * ```js
   * export default defineConfig({
   *   output: {
   *     sourcemap: true,
   *     sourcemapPathTransform: (source, sourcemapPath) => {
   *       // Remove 'src/' prefix from all source paths
   *       return source.replace(/^src\//, '');
   *     },
   *   },
   * });
   * ```
   */
  sourcemapPathTransform?: SourcemapPathTransformOption;
  /**
   * A string to prepend to the bundle before {@linkcode Plugin.renderChunk | renderChunk} hook.
   *
   * See {@linkcode intro | output.intro}, {@linkcode postBanner | output.postBanner} as well.
   *
   * {@include ./docs/output-banner.md}
   */
  banner?: string | AddonFunction;
  /**
   * A string to append to the bundle before {@linkcode Plugin.renderChunk | renderChunk} hook.
   *
   * See {@linkcode outro | output.outro}, {@linkcode postFooter | output.postFooter} as well.
   *
   * {@include ./docs/output-footer.md}
   */
  footer?: string | AddonFunction;
  /**
   * A string to prepend to the bundle after {@linkcode Plugin.renderChunk | renderChunk} hook and minification.
   *
   * See {@linkcode banner | output.banner}, {@linkcode intro | output.intro} as well.
   *
   * {@include ./docs/output-post-banner.md}
   */
  postBanner?: string | AddonFunction;
  /**
   * A string to append to the bundle after {@linkcode Plugin.renderChunk | renderChunk} hook and minification.
   *
   * See {@linkcode footer | output.footer}, {@linkcode outro | output.outro} as well.
   *
   * {@include ./docs/output-post-footer.md}
   */
  postFooter?: string | AddonFunction;
  /**
   * A string to prepend inside any {@link OutputOptions.format | format}-specific wrapper.
   *
   * See {@linkcode banner | output.banner}, {@linkcode postBanner | output.postBanner} as well.
   *
   * {@include ./docs/output-intro.md}
   */
  intro?: string | AddonFunction;
  /**
   * A string to append inside any {@link OutputOptions.format | format}-specific wrapper.
   *
   * See {@linkcode footer | output.footer}, {@linkcode postFooter | output.postFooter} as well.
   *
   * {@include ./docs/output-outro.md}
   */
  outro?: string | AddonFunction;
  /**
   * Whether to extend the global variable defined by the {@linkcode OutputOptions.name | name} option in `umd` or `iife` {@link OutputOptions.format | formats}.
   *
   * When `true`, the global variable will be defined as `global.name = global.name || {}`.
   * When `false`, the global defined by name will be overwritten like `global.name = {}`.
   *
   * @default false
   */
  extend?: boolean;
  /**
   * Whether to add a `__esModule: true` property when generating exports for non-ES {@link OutputOptions.format | formats}.
   *
   * This property signifies that the exported value is the namespace of an ES module and that the default export of this module corresponds to the `.default` property of the exported object.
   *
   * - `true`: Always add the property when using {@link OutputOptions.exports | named exports mode}, which is similar to what other tools do.
   * - `"if-default-prop"`: Only add the property when using {@link OutputOptions.exports | named exports mode} and there also is a default export. The subtle difference is that if there is no default export, consumers of the CommonJS version of your library will get all named exports as default export instead of an error or `undefined`.
   * - `false`: Never add the property even if the default export would become a property `.default`.
   *
   * @default 'if-default-prop'
   *
   * {@include ./docs/output-es-module.md}
   */
  esModule?: boolean | 'if-default-prop';
  /**
   * The pattern to use for naming custom emitted assets to include in the build output, or a function that is called per asset with {@linkcode PreRenderedAsset} to return such a pattern.
   *
   * Patterns support the following placeholders:
   * - `[extname]`: The file extension of the asset including a leading dot, e.g. `.css`.
   * - `[ext]`: The file extension without a leading dot, e.g. css.
   * - `[hash]`: A hash based on the content of the asset. You can also set a specific hash length via e.g. `[hash:10]`. By default, it will create a base-64 hash. If you need a reduced character set, see {@linkcode hashCharacters | output.hashCharacters}.
   * - `[name]`: The file name of the asset excluding any extension.
   *
   * Forward slashes (`/`) can be used to place files in sub-directories.
   *
   * See also {@linkcode chunkFileNames | output.chunkFileNames}, {@linkcode entryFileNames | output.entryFileNames}.
   *
   * @default 'assets/[name]-[hash][extname]'
   */
  assetFileNames?: string | AssetFileNamesFunction;
  /**
   * The pattern to use for chunks created from entry points, or a function that is called per entry chunk with {@linkcode PreRenderedChunk} to return such a pattern.
   *
   * Patterns support the following placeholders:
   * - `[format]`: The rendering format defined in the output options. The value is any of {@linkcode InternalModuleFormat}.
   * - `[hash]`: A hash based only on the content of the final generated chunk, including transformations in `renderChunk` and any referenced file hashes. You can also set a specific hash length via e.g. `[hash:10]`. By default, it will create a base-64 hash. If you need a reduced character set, see {@linkcode hashCharacters | output.hashCharacters}.
   * - `[name]`: The file name (without extension) of the entry point, unless the object form of input was used to define a different name.
   *
   * Forward slashes (`/`) can be used to place files in sub-directories. This pattern will also be used for every file when setting the {@linkcode preserveModules | output.preserveModules} option.
   *
   * See also {@linkcode assetFileNames | output.assetFileNames}, {@linkcode chunkFileNames | output.chunkFileNames}.
   *
   * @default '[name].js'
   */
  entryFileNames?: string | ChunkFileNamesFunction;
  /**
   * The pattern to use for naming shared chunks created when code-splitting, or a function that is called per chunk with {@linkcode PreRenderedChunk} to return such a pattern.
   *
   * Patterns support the following placeholders:
   * - `[format]`: The rendering format defined in the output options. The value is any of {@linkcode InternalModuleFormat}.
   * - `[hash]`: A hash based only on the content of the final generated chunk, including transformations in `renderChunk` and any referenced file hashes. You can also set a specific hash length via e.g. `[hash:10]`. By default, it will create a base-64 hash. If you need a reduced character set, see {@linkcode hashCharacters | output.hashCharacters}.
   * - `[name]`: The name of the chunk. This can be explicitly set via the {@linkcode codeSplitting | output.codeSplitting} option or when the chunk is created by a plugin via `this.emitFile`. Otherwise, it will be derived from the chunk contents.
   *
   * Forward slashes (`/`) can be used to place files in sub-directories.
   *
   * See also {@linkcode assetFileNames | output.assetFileNames}, {@linkcode entryFileNames | output.entryFileNames}.
   *
   * @default '[name]-[hash].js'
   */
  chunkFileNames?: string | ChunkFileNamesFunction;
  /**
   * @default '[name].css'
   * @experimental
   * @hidden not ready for public usage yet
   */
  cssEntryFileNames?: string | ChunkFileNamesFunction;
  /**
   * @default '[name]-[hash].css'
   * @experimental
   * @hidden not ready for public usage yet
   */
  cssChunkFileNames?: string | ChunkFileNamesFunction;
  /**
   * Whether to enable chunk name sanitization (removal of non-URL-safe characters like `\0`, `?` and `*`).
   *
   * Set `false` to disable the sanitization. You can also provide a custom sanitization function.
   *
   * @default true
   */
  sanitizeFileName?: boolean | SanitizeFileNameFunction;
  /**
   * Control code minification
   *
   * Rolldown uses Oxc Minifier under the hood. See Oxc's [minification documentation](https://oxc.rs/docs/guide/usage/minifier#features) for more details.
   *
   * - `true`: Enable full minification including code compression and dead code elimination
   * - `false`: Disable minification (default)
   * - `'dce-only'`: Only perform dead code elimination without code compression
   * - `MinifyOptions`: Fine-grained control over minification settings
   *
   * @default false
   */
  minify?: boolean | 'dce-only' | MinifyOptions;
  /**
   * Specifies the global variable name that contains the exports of `umd` / `iife` {@link OutputOptions.format | formats}.
   *
   * @example
   * ```js
   * export default defineConfig({
   *   output: {
   *     format: 'iife',
   *     name: 'MyBundle',
   *   }
   * });
   * ```
   * ```js
   * // output
   * var MyBundle = (function () {
   *   // ...
   * })();
   * ```
   *
   * {@include ./docs/output-name.md}
   */
  name?: string;
  /**
   * Specifies `id: variableName` pairs necessary for {@link InputOptions.external | external} imports in `umd` / `iife` {@link OutputOptions.format | formats}.
   *
   * @example
   * ```js
   * export default defineConfig({
   *   external: ['jquery'],
   *   output: {
   *     format: 'iife',
   *     name: 'MyBundle',
   *     globals: {
   *       jquery: '$',
   *     }
   *   }
   * });
   * ```
   * ```js
   * // input
   * import $ from 'jquery';
   * ```
   * ```js
   * // output
   * var MyBundle = (function ($) {
   *   // ...
   * })($);
   * ```
   */
  globals?: Record<string, string> | GlobalsFunction;
  /**
   * Maps {@link InputOptions.external | external} module IDs to paths.
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
  /**
   * Which language features Rolldown can safely use in generated code.
   *
   * This will not transpile any user code but only change the code Rolldown uses in wrappers and helpers.
   */
  generatedCode?: Partial<GeneratedCodeOptions>;
  /**
   * Whether to generate code to support live bindings for {@link InputOptions.external | external} imports.
   *
   * With the default value of `true`, Rolldown will generate code to support live bindings for external imports.
   *
   * When set to `false`, Rolldown will assume that exports from external modules do not change. This will allow Rolldown to generate smaller code. Note that this can cause issues when there are circular dependencies involving an external dependency.
   *
   * @default true
   *
   * {@include ./docs/output-external-live-bindings.md}
   */
  externalLiveBindings?: boolean;
  /**
   * @deprecated Please use `codeSplitting: false` instead.
   *
   * Whether to inline dynamic imports instead of creating new chunks to create a single bundle.
   *
   * This option can be used only when a single input is provided.
   *
   * @default false
   */
  inlineDynamicImports?: boolean;
  /**
   * Whether to keep external dynamic imports as `import(...)` expressions in CommonJS output.
   *
   * If set to `false`, external dynamic imports will be rewritten to use `require(...)` calls.
   * This may be necessary to support environments that do not support dynamic `import()` in CommonJS modules like old Node.js versions.
   *
   * @default true
   */
  dynamicImportInCjs?: boolean;
  /**
   * Allows you to do manual chunking. Provided for Rollup compatibility.
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
   *   codeSplitting: {
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
   * Note that unlike Rollup, object form is not supported.
   *
   * @deprecated
   * Please use {@linkcode codeSplitting | output.codeSplitting} instead.
   *
   * :::warning
   * If `manualChunks` and `codeSplitting` are both specified, `manualChunks` option will be ignored.
   * :::
   */
  manualChunks?: ManualChunksFunction;
  /**
   * Controls how code splitting is performed.
   *
   * - `true`: Default behavior, automatic code splitting. **(default)**
   * - `false`: Inline all dynamic imports into a single bundle (equivalent to deprecated `inlineDynamicImports: true`).
   * - `object`: Advanced manual code splitting configuration.
   *
   * For deeper understanding, please refer to the in-depth [documentation](https://rolldown.rs/in-depth/manual-code-splitting).
   *
   * @example
   * **Basic vendor chunk**
   * ```js
   * export default defineConfig({
   *   output: {
   *     codeSplitting: {
   *       minSize: 20000,
   *       groups: [
   *         {
   *           name: 'vendor',
   *           test: /node_modules/,
   *         },
   *       ],
   *     },
   *   },
   * });
   * ```
   * {@include ./docs/output-code-splitting.md}
   *
   * @default true
   */
  codeSplitting?: boolean | CodeSplittingOptions;
  /**
   * @deprecated Please use {@linkcode codeSplitting | output.codeSplitting} instead.
   *
   * Allows you to do manual chunking.
   *
   * :::warning
   * If `advancedChunks` and `codeSplitting` are both specified, `advancedChunks` option will be ignored.
   * :::
   */
  advancedChunks?: {
    includeDependenciesRecursively?: boolean;
    minSize?: number;
    maxSize?: number;
    maxModuleSize?: number;
    minModuleSize?: number;
    minShareCount?: number;
    groups?: CodeSplittingGroup[];
  };
  /**
   * Control comments in the output.
   *
   * - `none`: no comments
   * - `inline`: preserve comments that contain `@license`, `@preserve` or starts with `//!` `/*!`
   */
  legalComments?: 'none' | 'inline';
  /**
   * The list of plugins to use only for this output.
   *
   * @see {@linkcode InputOptions.plugins | plugins}
   */
  plugins?: RolldownOutputPluginOption;
  /**
   * Whether to add a polyfill for `require()` function in non-CommonJS formats.
   *
   * This option is useful when you want to inject your own `require` implementation.
   *
   * @default true
   */
  polyfillRequire?: boolean;
  /**
   * This option is not implemented yet.
   * @hidden
   */
  hoistTransitiveImports?: false;
  /**
   * Whether to use preserve modules mode.
   *
   * {@include ./docs/output-preserve-modules.md}
   *
   * @default false
   */
  preserveModules?: boolean;
  /**
   * Specifies the directory name for "virtual" files that might be emitted by plugins when using {@link OutputOptions.preserveModules | preserve modules mode}.
   *
   * @default '_virtual'
   */
  virtualDirname?: string;
  /**
   * A directory path to input modules that should be stripped away from {@linkcode dir | output.dir} when using {@link OutputOptions.preserveModules | preserve modules mode}.
   *
   * {@include ./docs/output-preserve-modules-root.md}
   */
  preserveModulesRoot?: string;
  /**
   * Whether to use `var` declarations at the top level scope instead of function / class / let / const expressions.
   *
   * Enabling this option can improve runtime performance of the generated code in certain environments.
   *
   * @default false
   *
   * {@include ./docs/output-top-level-var.md}
   */
  topLevelVar?: boolean;
  /**
   * Whether to minify internal exports as single letter variables to allow for better minification.
   *
   * @default
   * `true` for format `es` or if `output.minify` is `true` or object, `false` otherwise
   *
   * {@include ./docs/output-minify-internal-exports.md}
   */
  minifyInternalExports?: boolean;
  /**
   * Clean output directory ({@linkcode dir | output.dir}) before emitting output.
   *
   * @default false
   *
   * {@include ./docs/output-clean-dir.md}
   */
  cleanDir?: boolean;
  /**
   * Keep `name` property of functions and classes after bundling.
   *
   * When enabled, the bundler will preserve the original `name` property value of functions and
   * classes in the output. This is useful for debugging and some frameworks that rely on it for
   * registration and binding purposes.
   *
   * {@include ./docs/output-keep-names.md}
   *
   * @default false
   */
  keepNames?: boolean;
  /**
   * Lets modules be executed in the order they are declared.
   *
   * This is done by injecting runtime helpers to ensure that modules are executed in the order they are imported. External modules won't be affected.
   *
   * > [!WARNING]
   * > Enabling this option may negatively increase bundle size. It is recommended to use this option only when absolutely necessary.
   * @default false
   */
  strictExecutionOrder?: boolean;
}

export type CodeSplittingGroup = {
  /**
   * Name of the group. It will be also used as the name of the chunk and replace the `[name]` placeholder in the {@linkcode OutputOptions.chunkFileNames | output.chunkFileNames} option.
   *
   * For example,
   *
   * ```js
   * import { defineConfig } from 'rolldown';
   *
   * export default defineConfig({
   *   output: {
   *     codeSplitting: {
   *       groups: [
   *         {
   *           name: 'libs',
   *           test: /node_modules/,
   *         },
   *       ],
   *     },
   *   },
   * });
   * ```
   * will create a chunk named `libs-[hash].js` in the end.
   *
   * It's ok to have the same name for different groups. Rolldown will deduplicate the chunk names if necessary.
   *
   * #### Dynamic `name()`
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
   *   output: {
   *     codeSplitting: {
   *       groups: [
   *         {
   *           name: (moduleId) => moduleId.includes('node_modules') ? 'libs' : 'app',
   *           minSize: 100 * 1024,
   *         },
   *       ],
   *     },
   *   },
   * });
   * ```
   *
   * :::warning
   * Constraints like `minSize`, `maxSize`, etc. are applied separately for different names returned by the function.
   * :::
   */
  name: string | CodeSplittingNameFunction;
  /**
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
  test?: StringOrRegExp | CodeSplittingTestFunction;
  /**
   * Priority of the group. Group with higher priority will be chosen first to match modules and create chunks. When converting the group to a chunk, modules of that group will be removed from other groups.
   *
   * If two groups have the same priority, the group whose index is smaller will be chosen.
   *
   * @example
   * ```js
   * import { defineConfig } from 'rolldown';
   *
   * export default defineConfig({
   *   output: {
   *     codeSplitting: {
   *       groups: [
   *         {
   *           name: 'react',
   *           test: /node_modules[\\/]react/,
   *           priority: 2,
   *         },
   *         {
   *           name: 'other-libs',
   *           test: /node_modules/,
   *           priority: 1,
   *         },
   *       ],
   *     },
   *   },
   * });
   * ```
   *
   * @default 0
   */
  priority?: number;
  /**
   * Minimum size in bytes of the desired chunk. If the accumulated size of the captured modules by this group is smaller than this value, it will be ignored. Modules in this group will fall back to the `automatic chunking` if they are not captured by any other group.
   *
   * @default 0
   */
  minSize?: number;
  /**
   * Controls if a module should be captured based on how many entry chunks reference it.
   *
   * @default 1
   */
  minShareCount?: number;
  /**
   * If the accumulated size in bytes of the captured modules by this group is larger than this value, this group will be split into multiple groups that each has size close to this value.
   *
   * @default Infinity
   */
  maxSize?: number;
  /**
   * Controls whether a module can only be captured if its size in bytes is smaller than or equal to this value.
   *
   * @default Infinity
   */
  maxModuleSize?: number;
  /**
   * Controls whether a module can only be captured if its size in bytes is larger than or equal to this value.
   *
   * @default 0
   */
  minModuleSize?: number;
};

/**
 * Alias for {@linkcode CodeSplittingGroup}. Use this type for the `codeSplitting.groups` option.
 *
 * @deprecated Please use {@linkcode CodeSplittingGroup} instead.
 */
export type AdvancedChunksGroup = CodeSplittingGroup;

/**
 * Configuration options for advanced code splitting.
 */
export type CodeSplittingOptions = {
  /**
   * By default, each group will also include captured modules' dependencies. This reduces the chance of generating circular chunks.
   *
   * If you want to disable this behavior, it's recommended to both set
   * - {@linkcode InputOptions.preserveEntrySignatures | preserveEntrySignatures}: `false | 'allow-extension'`
   * - {@linkcode OutputOptions.strictExecutionOrder | strictExecutionOrder}: `true`
   *
   * to avoid generating invalid chunks.
   *
   * @default true
   */
  includeDependenciesRecursively?: boolean;
  /**
   * Global fallback of {@linkcode CodeSplittingGroup.minSize | group.minSize}, if it's not specified in the group.
   */
  minSize?: number;
  /**
   * Global fallback of {@linkcode CodeSplittingGroup.maxSize | group.maxSize}, if it's not specified in the group.
   */
  maxSize?: number;
  /**
   * Global fallback of {@linkcode CodeSplittingGroup.maxModuleSize | group.maxModuleSize}, if it's not specified in the group.
   */
  maxModuleSize?: number;
  /**
   * Global fallback of {@linkcode CodeSplittingGroup.minModuleSize | group.minModuleSize}, if it's not specified in the group.
   */
  minModuleSize?: number;
  /**
   * Global fallback of {@linkcode CodeSplittingGroup.minShareCount | group.minShareCount}, if it's not specified in the group.
   */
  minShareCount?: number;
  /**
   * Groups to be used for code splitting.
   */
  groups?: CodeSplittingGroup[];
};

/**
 * Alias for {@linkcode CodeSplittingOptions}. Use this type for the `codeSplitting` option.
 *
 * @deprecated Please use {@linkcode CodeSplittingOptions} instead.
 */
export type AdvancedChunksOptions = CodeSplittingOptions;

interface OverwriteOutputOptionsForCli {
  banner?: string;
  footer?: string;
  postBanner?: string;
  postFooter?: string;
  intro?: string;
  outro?: string;
  esModule?: boolean;
  globals?: Record<string, string>;
  codeSplitting?:
    | boolean
    | {
        minSize?: number;
        minShareCount?: number;
      };
  advancedChunks?: {
    minSize?: number;
    minShareCount?: number;
  };
}

export type OutputCliOptions = Omit<
  OutputOptions,
  keyof OverwriteOutputOptionsForCli | 'sourcemapIgnoreList' | 'sourcemapPathTransform'
> &
  OverwriteOutputOptionsForCli;
