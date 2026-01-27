import type {
  LogLevel,
  LogLevelOption,
  LogOrStringHandler,
  RolldownLog,
  RolldownLogWithString,
} from '../log/logging';
import type { RolldownPluginOption } from '../plugin';
import type { TreeshakingOptions } from '../types/module-side-effects';
import type { NullValue, StringOrRegExp } from '../types/utils';
import type { ChecksOptions } from './generated/checks-options';
import type { TransformOptions } from './transform-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { watch } from '../api/watch/index';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { Plugin } from '../plugin';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { RolldownBuild } from '../api/rolldown/rolldown-build';

/**
 * @inline
 */
export type InputOption = string | string[] | Record<string, string>;

/**
 * @param id The id of the module being checked.
 * @param parentId The id of the module importing the id being checked.
 * @param isResolved Whether the id has been resolved.
 * @returns Whether the module should be treated as external.
 */
export type ExternalOptionFunction = (
  id: string,
  parentId: string | undefined,
  isResolved: boolean,
) => NullValue<boolean>;

/** @inline */
export type ExternalOption = StringOrRegExp | StringOrRegExp[] | ExternalOptionFunction;

export type ModuleTypes = Record<
  string,
  | 'js'
  | 'jsx'
  | 'ts'
  | 'tsx'
  | 'json'
  | 'text'
  | 'base64'
  | 'dataurl'
  | 'binary'
  | 'empty'
  | 'css'
  | 'asset'
>;

export interface WatcherOptions {
  /**
   * Whether to skip the {@linkcode RolldownBuild.write | bundle.write()} step when a rebuild is triggered.
   * @default false
   */
  skipWrite?: boolean;
  /**
   * Configures how long Rolldown will wait for further changes until it triggers
   * a rebuild in milliseconds.
   *
   * Even if this value is set to 0, there's a small debounce timeout configured
   * in the file system watcher. Setting this to a value greater than 0 will mean
   * that Rolldown will only trigger a rebuild if there was no change for the
   * configured number of milliseconds. If several configurations are watched,
   * Rolldown will use the largest configured build delay.
   *
   * This option is useful if you use a tool that regenerates multiple source files
   * very slowly. Rebuilding immediately after the first change could cause Rolldown
   * to generate a broken intermediate build before generating a successful final
   * build, which can be confusing and distracting.
   *
   * @default 0
   */
  buildDelay?: number;
  /**
   * An optional object of options that will be passed to the [notify](https://github.com/rolldown/notify) file watcher.
   */
  notify?: {
    /**
     * Interval between each re-scan attempt in milliseconds.
     *
     * This option is only used when polling backend is used.
     *
     * @default 30_000
     */
    pollInterval?: number;
    /**
     * Whether to compare file contents when checking for changes.
     *
     * This is especially important for pseudo filesystems like those on Linux
     * under `/sys` and `/proc` which are not obligated to respect any other
     * filesystem norms such as modification timestamps, file sizes, etc. By
     * enabling this feature, performance will be significantly impacted as
     * all files will need to be read and hashed at each interval.
     *
     * This option is only used when polling backend is used.
     *
     * @default false
     */
    compareContents?: boolean;
  };
  /**
   * Filter to limit the file-watching to certain files.
   *
   * Strings are treated as glob patterns.
   * Note that this only filters the module graph but does not allow adding
   * additional watch files.
   *
   * @example
   * ```js
   * export default defineConfig({
   *   watch: {
   *     include: 'src/**',
   *   },
   * })
   * ```
   * @default []
   */
  include?: StringOrRegExp | StringOrRegExp[];
  /**
   * Filter to prevent files from being watched.
   *
   * Strings are treated as glob patterns.
   *
   * @example
   * ```js
   * export default defineConfig({
   *   watch: {
   *     exclude: 'node_modules/**',
   *   },
   * })
   * ```
   * @default []
   */
  exclude?: StringOrRegExp | StringOrRegExp[];
  /**
   * An optional function that will be called immediately every time
   * a module changes that is part of the build.
   *
   * This is different from the {@linkcode Plugin.watchChange | watchChange} plugin hook, which is
   * only called once the running build has finished. This may for
   * instance be used to prevent additional steps from being performed
   * if we know another build will be started anyway once the current
   * build finished. This callback may be called multiple times per
   * build as it tracks every change.
   *
   * @param id The id of the changed module.
   */
  onInvalidate?: (id: string) => void;
  /**
   * Whether to clear the screen when a rebuild is triggered.
   * @default true
   */
  clearScreen?: boolean;
}

/** @inline */
type MakeAbsoluteExternalsRelative = boolean | 'ifRelativeSource';

export type DevModeOptions =
  | boolean
  | {
      host?: string;
      port?: number;
      implement?: string;
      lazy?: boolean;
    };

export type OptimizationOptions = {
  /**
   * Inline imported constant values during bundling instead of preserving variable references.
   *
   * When enabled, constant values from imported modules will be inlined at their usage sites,
   * potentially reducing bundle size and improving runtime performance by eliminating variable lookups.
   *
   * **Options:**
   * - `true`: equivalent to `{ mode: 'all', pass: 1 }`, enabling constant inlining for all eligible constants with a single pass.
   * - `false`: Disable constant inlining
   * - `{ mode: 'smart' | 'all', pass?: number }`:
   *   - `mode: 'smart'`: Only inline constants in specific scenarios where it is likely to reduce bundle size and improve performance.
   *     Smart mode inlines constants in these specific scenarios:
   *     1. `if (test) {} else {}` - condition expressions in if statements
   *     2. `test ? a : b` - condition expressions in ternary operators
   *     3. `test1 || test2` - logical OR expressions
   *     4. `test1 && test2` - logical AND expressions
   *     5. `test1 ?? test2` - nullish coalescing expressions
   *  - `mode: 'all'`: Inline all imported constants wherever they are used.
   *  - `pass`: Number of passes to perform for inlining constants.
   *
   * @example
   * ```js
   * // Input files:
   * // constants.js
   * export const API_URL = 'https://api.example.com';
   *
   * // main.js
   * import { API_URL } from './constants.js';
   * console.log(API_URL);
   *
   * // With inlineConst: true, the bundled output becomes:
   * console.log('https://api.example.com');
   *
   * // Instead of:
   * const API_URL = 'https://api.example.com';
   * console.log(API_URL);
   * ```
   *
   * @default false
   */
  inlineConst?: boolean | { mode?: 'all' | 'smart'; pass?: number };

  /**
   * Use PIFE pattern for module wrappers.
   *
   * Enabling this option improves the start up performance of the generated bundle with the cost of a slight increase in bundle size.
   *
   * {@include ./docs/optimization-pife-for-module-wrappers.md}
   *
   * @default true
   */
  pifeForModuleWrappers?: boolean;
};

/** @inline */
export type AttachDebugOptions = 'none' | 'simple' | 'full';

/** @inline */
type ChunkModulesOrder = 'exec-order' | 'module-id';

/** @inline */
export type OnLogFunction = (
  level: LogLevel,
  log: RolldownLog,
  defaultHandler: LogOrStringHandler,
) => void;

/** @inline */
export type OnwarnFunction = (
  warning: RolldownLog,
  defaultHandler: (warning: RolldownLogWithString | (() => RolldownLogWithString)) => void,
) => void;

export interface InputOptions {
  /**
   * Defines entries and location(s) of entry modules for the bundle. Relative paths are resolved based on the {@linkcode cwd} option.
   * {@include ./docs/input.md}
   */
  input?: InputOption;
  /**
   * The list of plugins to use.
   *
   * Falsy plugins will be ignored, which can be used to easily activate or deactivate plugins. Nested plugins will be flattened. Async plugins will be awaited and resolved.
   *
   * See [Plugin API document](https://rolldown.rs/apis/plugin-api) for more details about creating plugins.
   *
   * @example
   * ```js
   * import { defineConfig } from 'rolldown'
   *
   * export default defineConfig({
   *   plugins: [
   *     examplePlugin1(),
   *     // Conditional plugins
   *     process.env.ENV1 && examplePlugin2(),
   *     // Nested plugins arrays are flattened
   *     [examplePlugin3(), examplePlugin4()],
   *   ]
   * })
   * ```
   */
  plugins?: RolldownPluginOption;
  /**
   * Specifies which modules should be treated as external and not bundled. External modules will be left as import statements in the output.
   * {@include ./docs/external.md}
   */
  external?: ExternalOption;
  /**
   * Options for built-in module resolution feature.
   */
  resolve?: {
    /**
     * Substitute one package for another.
     *
     * One use case for this feature is replacing a node-only package with a browser-friendly package in third-party code that you don't control.
     *
     * @example
     * ```js
     * resolve: {
     *   alias: {
     *     '@': '/src',
     *     'utils': './src/utils',
     *   }
     * }
     * ```
     * > [!WARNING]
     * > `resolve.alias` will not call [`resolveId`](/reference/Interface.Plugin#resolveid) hooks of other plugin.
     * > If you want to call `resolveId` hooks of other plugin, use `viteAliasPlugin` from `rolldown/experimental` instead.
     * > You could find more discussion in [this issue](https://github.com/rolldown/rolldown/issues/3615)
     */
    alias?: Record<string, string[] | string | false>;
    /**
     * Fields in package.json to check for aliased paths.
     *
     * This option is expected to be used for `browser` field support.
     *
     * @default
     * - `[['browser']]` for `browser` platform
     * - `[]` for other platforms
     */
    aliasFields?: string[][];
    /**
     * Condition names to use when resolving exports in package.json.
     *
     * @default
     * Defaults based on platform and import kind:
     * - `browser` platform
     *   - `["import", "browser", "default"]` for import statements
     *   - `["require", "browser", "default"]` for require() calls
     * - `node` platform
     *   - `["import", "node", "default"]` for import statements
     *   - `["require", "node", "default"]` for require() calls
     * - `neutral` platform
     *   - `["import", "default"]` for import statements
     *   - `["require", "default"]` for require() calls
     */
    conditionNames?: string[];
    /**
     * Map of extensions to alternative extensions.
     *
     * With writing `import './foo.js'` in a file, you want to resolve it to `foo.ts` instead of `foo.js`.
     * You can achieve this by setting: `extensionAlias: { '.js': ['.ts', '.js'] }`.
     */
    extensionAlias?: Record<string, string[]>;
    /**
     * Fields in package.json to check for exports.
     *
     * @default `[['exports']]`
     */
    exportsFields?: string[][];
    /**
     * Extensions to try when resolving files. These are tried in order from first to last.
     *
     * @default `['.tsx', '.ts', '.jsx', '.js', '.json']`
     */
    extensions?: string[];
    /**
     * Fields in package.json to check for entry points.
     *
     * @default
     * Defaults based on platform:
     * - `node` platform: `['main', 'module']`
     * - `browser` platform: `['browser', 'module', 'main']`
     * - `neutral` platform: `[]`
     */
    mainFields?: string[];
    /**
     * Filenames to try when resolving directories.
     * @default ['index']
     */
    mainFiles?: string[];
    /**
     * Directories to search for modules.
     * @default ['node_modules']
     */
    modules?: string[];
    /**
     * Whether to follow symlinks when resolving modules.
     * @default true
     */
    symlinks?: boolean;
    /**
     * @deprecated Use the top-level {@linkcode tsconfig} option instead.
     */
    tsconfigFilename?: string;
  };
  /**
   * The working directory to use when resolving relative paths in the configuration.
   * @default process.cwd()
   */
  cwd?: string;
  /**
   * Expected platform where the code run.
   *
   *  When the platform is set to neutral:
   *    - When bundling is enabled the default output format is set to esm, which uses the export syntax introduced with ECMAScript 2015 (i.e. ES6). You can change the output format if this default is not appropriate.
   *    - The main fields setting is empty by default. If you want to use npm-style packages, you will likely have to configure this to be something else such as main for the standard main field used by node.
   *    - The conditions setting does not automatically include any platform-specific values.
   *
   * @default
   * - `'node'` if the format is `'cjs'`
   * - `'browser'` for other formats
   * {@include ./docs/platform.md}
   */
  platform?: 'node' | 'browser' | 'neutral';
  /**
   * When `true`, creates shim variables for missing exports instead of throwing an error.
   * @default false
   * {@include ./docs/shim-missing-exports.md}
   */
  shimMissingExports?: boolean;
  /**
   * Controls tree-shaking (dead code elimination).
   *
   * See the [In-depth Dead Code Elimination Guide](https://rolldown.rs/in-depth/dead-code-elimination) for more details.
   *
   * When `false`, tree-shaking will be disabled.
   * When `true`, it is equivalent to setting each options to the default value.
   *
   * @default true
   */
  treeshake?: boolean | TreeshakingOptions;
  /**
   * Controls the verbosity of console logging during the build.
   *
   * {@include ./docs/log-level.md}
   *
   * @default 'info'
   */
  logLevel?: LogLevelOption;
  /**
   * A function that intercepts log messages. If not supplied, logs are printed to the console.
   *
   * {@include ./docs/on-log.md}
   *
   * @example
   * ```js
   * export default defineConfig({
   *   onLog(level, log, defaultHandler) {
   *     if (log.code === 'CIRCULAR_DEPENDENCY') {
   *       return; // Ignore circular dependency warnings
   *     }
   *     if (level === 'warn') {
   *       defaultHandler('error', log); // turn other warnings into errors
   *     } else {
   *       defaultHandler(level, log); // otherwise, just print the log
   *     }
   *   }
   * })
   * ```
   */
  onLog?: OnLogFunction;
  /**
   * A function that will intercept warning messages.
   *
   * {@include ./docs/on-warn.md}
   *
   * @deprecated
   * This is a legacy API. Consider using {@linkcode onLog} instead for better control over all log types.
   *
   * {@include ./docs/on-warn-deprecation.md}
   */
  onwarn?: OnwarnFunction;
  /**
   * Maps file patterns to module types, controlling how files are processed.
   *
   * This is conceptually similar to [esbuild's `loader`](https://esbuild.github.io/api/#loader) option, allowing you to specify how each file extensions should be handled.
   *
   * See [the In-Depth Guide](https://rolldown.rs/in-depth/module-types) for more details.
   *
   * @example
   * ```js
   * import { defineConfig } from 'rolldown'
   *
   * export default defineConfig({
   *   moduleTypes: {
   *     '.frag': 'text',
   *   }
   * })
   * ```
   */
  moduleTypes?: ModuleTypes;
  /**
   * Experimental features that may change in future releases and can introduce behavior change without a major version bump.
   * @experimental
   */
  experimental?: {
    /**
     * Enable Vite compatible mode.
     * @default false
     * @hidden This option is only meant to be used by Vite. It is not recommended to use this option directly.
     */
    viteMode?: boolean;
    /**
     * When enabled, `new URL()` calls will be transformed to a stable asset URL which includes the updated name and content hash.
     * It is necessary to pass `import.meta.url` as the second argument to the
     * `new URL` constructor, otherwise no transform will be applied.
     * :::warning
     * JavaScript and TypeScript files referenced via `new URL('./file.js', import.meta.url)` or `new URL('./file.ts', import.meta.url)` will **not** be transformed or bundled. The file will be copied as-is, meaning TypeScript files remain untransformed and dependencies are not resolved.
     *
     * The expected behavior for JS/TS files is still being discussed and may
     * change in future releases. See [#7258](https://github.com/rolldown/rolldown/issues/7258) for more context.
     * :::
     * @example
     * ```js
     * // main.js
     * const url = new URL('./styles.css', import.meta.url);
     * console.log(url);
     *
     * // Example output after bundling WITHOUT the option (default)
     * const url = new URL('./styles.css', import.meta.url);
     * console.log(url);
     *
     * // Example output after bundling WITH `experimental.resolveNewUrlToAsset` set to `true`
     * const url = new URL('assets/styles-CjdrdY7X.css', import.meta.url);
     * console.log(url);
     * ```
     * @default false
     */
    resolveNewUrlToAsset?: boolean;
    /**
     * Dev mode related options.
     * @hidden not ready for public usage yet
     */
    devMode?: DevModeOptions;
    /**
     * Control which order should be used when rendering modules in a chunk.
     *
     * Available options:
     * - `exec-order`: Almost equivalent to the topological order of the module graph, but specially handling when module graph has cycle.
     * - `module-id`: This is more friendly for gzip compression, especially for some javascript static asset lib (e.g. icon library)
     *
     * > [!NOTE]
     * > Try to sort the modules by their module id if possible (Since rolldown scope hoist all modules in the chunk, we only try to sort those modules by module id if we could ensure runtime behavior is correct after sorting).
     *
     * @default 'exec-order'
     */
    chunkModulesOrder?: ChunkModulesOrder;
    /**
     * Attach debug information to the output bundle.
     *
     * Available modes:
     * - `none`: No debug information is attached.
     * - `simple`: Attach comments indicating which files the bundled code comes from. These comments could be removed by the minifier.
     * - `full`: Attach detailed debug information to the output bundle. These comments are using legal comment syntax, so they won't be removed by the minifier.
     *
     * @default 'simple'
     *
     * {@include ./docs/experimental-attach-debug-info.md}
     */
    attachDebugInfo?: AttachDebugOptions;
    /**
     * Enables automatic generation of a chunk import map asset during build.
     *
     * This map only includes chunks with hashed filenames, where keys are derived from the facade module
     * name or primary chunk name. It produces stable and unique hash-based filenames, effectively preventing
     * cascading cache invalidation caused by content hashes and maximizing browser cache reuse.
     *
     * The output defaults to `importmap.json` unless overridden via `fileName`. A base URL prefix
     * (default `"/"`) can be applied to all paths. The resulting JSON is a valid import map and can be
     * directly injected into HTML via `<script type="importmap">`.
     *
     * @example
     * ```js
     * {
     *   experimental: {
     *     chunkImportMap: {
     *       baseUrl: '/',
     *       fileName: 'importmap.json'
     *     }
     *   },
     *   plugins: [
     *     {
     *       name: 'inject-import-map',
     *       generateBundle(_, bundle) {
     *         const chunkImportMap = bundle['importmap.json'];
     *         if (chunkImportMap?.type === 'asset') {
     *           const htmlPath = path.resolve('index.html');
     *           let html = fs.readFileSync(htmlPath, 'utf-8');
     *
     *           html = html.replace(
     *             /<script\s+type="importmap"[^>]*>[\s\S]*?<\/script>/i,
     *             `<script type="importmap">${chunkImportMap.source}</script>`
     *           );
     *
     *           fs.writeFileSync(htmlPath, html);
     *           delete bundle['importmap.json'];
     *         }
     *       }
     *     }
     *   ]
     * }
     * ```
     *
     * > [!TIP]
     * > If you want to learn more, you can check out the example here: [examples/chunk-import-map](https://github.com/rolldown/rolldown/tree/main/examples/chunk-import-map)
     *
     * @default false
     */
    chunkImportMap?: boolean | { baseUrl?: string; fileName?: string };
    /**
     * Enable on-demand wrapping of modules.
     * @default false
     * @hidden not ready for public usage yet
     */
    onDemandWrapping?: boolean;
    /**
     * Enable incremental build support. Required to be used with `watch` mode.
     * @default false
     */
    incrementalBuild?: boolean;
    /**
     * Use native Rust implementation of MagicString for source map generation.
     *
     * [MagicString](https://github.com/rich-harris/magic-string) is a JavaScript library commonly used by bundlers
     * for string manipulation and source map generation. When enabled, rolldown will use a native Rust
     * implementation of MagicString instead of the JavaScript version, providing significantly better performance
     * during source map generation and code transformation.
     *
     * **Benefits**
     *
     * - **Improved Performance**: The native Rust implementation is typically faster than the JavaScript version,
     *   especially for large codebases with extensive source maps.
     * - **Background Processing**: Source map generation is performed asynchronously in a background thread,
     *   allowing the main bundling process to continue without blocking. This parallel processing can significantly
     *   reduce overall build times when working with JavaScript transform hooks.
     * - **Better Integration**: Seamless integration with rolldown's native Rust architecture.
     *
     * @example
     * ```js
     * export default {
     *   experimental: {
     *     nativeMagicString: true
     *   },
     *   output: {
     *     sourcemap: true
     *   }
     * }
     * ```
     *
     * > [!NOTE]
     * > This is an experimental feature. While it aims to provide identical behavior to the JavaScript
     * > implementation, there may be edge cases. Please report any discrepancies you encounter.
     * > For a complete working example, see [examples/native-magic-string](https://github.com/rolldown/rolldown/tree/main/examples/native-magic-string)
     * @default false
     */
    nativeMagicString?: boolean;
    /**
     * Control whether to optimize chunks by allowing entry chunks to have different exports than the underlying entry module.
     * This optimization can reduce the number of generated chunks.
     *
     * When enabled, rolldown will try to insert common modules directly into existing chunks rather than creating
     * separate chunks for them, which can result in fewer output files and better performance.
     *
     * This optimization is automatically disabled when any module uses top-level await (TLA) or contains TLA dependencies,
     * as it could affect execution order guarantees.
     *
     * @default true
     */
    chunkOptimization?: boolean;
    /**
     * Control whether to enable lazy barrel optimization.
     *
     * Lazy barrel optimization avoids compiling unused re-export modules in side-effect-free barrel modules,
     * significantly improving build performance for large codebases with many barrel modules.
     *
     * @see {@link https://rolldown.rs/in-depth/lazy-barrel-optimization | Lazy Barrel Documentation}
     * @default false
     */
    lazyBarrel?: boolean;
  };
  /**
   * Configure how the code is transformed. This process happens after the `transform` hook.
   *
   * @example
   * **Enable legacy decorators**
   * ```js
   * export default defineConfig({
   *   transform: {
   *     decorator: {
   *       legacy: true,
   *     },
   *   },
   * })
   * ```
   * Note that if you have correct `tsconfig.json` file, Rolldown will automatically detect and enable legacy decorators support.
   *
   * {@include ./docs/transform.md}
   */
  transform?: TransformOptions;
  /**
   * Watch mode related options.
   *
   * These options only take effect when running with the [`--watch`](/apis/cli#w-watch) flag, or using {@linkcode watch | watch()} API.
   *
   * @experimental
   */
  watch?: WatcherOptions | false;
  /**
   * Controls which warnings are emitted during the build process. Each option can be set to `true` (emit warning) or `false` (suppress warning).
   */
  checks?: ChecksOptions;
  /**
   * Determines if absolute external paths should be converted to relative paths in the output.
   *
   * This does not only apply to paths that are absolute in the source but also to paths that are resolved to an absolute path by either a plugin or Rolldown core.
   *
   * {@include ./docs/make-absolute-externals-relative.md}
   */
  makeAbsoluteExternalsRelative?: MakeAbsoluteExternalsRelative;
  /**
   * Devtools integration options.
   * @experimental
   */
  devtools?: {
    sessionId?: string;
  };
  /**
   * Controls how entry chunk exports are preserved.
   *
   * This determines whether Rolldown needs to create facade chunks (additional wrapper chunks) to maintain the exact export signatures of entry modules, or whether it can combine entry modules with other chunks for optimization.
   *
   * @default `'exports-only'`
   * {@include ./docs/preserve-entry-signatures.md}
   */
  preserveEntrySignatures?: false | 'strict' | 'allow-extension' | 'exports-only';
  /**
   * Configure optimization features for the bundler.
   */
  optimization?: OptimizationOptions;
  /**
   * The value of `this` at the top level of each module. **Normally, you don't need to set this option.**
   * @default undefined
   * @example
   * **Set custom context**
   * ```js
   * export default {
   *   context: 'globalThis',
   *   output: {
   *     format: 'iife',
   *   },
   * };
   * ```
   * {@include ./docs/context.md}
   */
  context?: string;
  /**
   * Configures TypeScript configuration file resolution and usage.
   * {@include ./docs/tsconfig.md}
   * @default true
   */
  tsconfig?: boolean | string;
}

interface OverwriteInputOptionsForCli {
  external?: string[];
  inject?: Record<string, string>;
  treeshake?: boolean;
}

export type InputCliOptions = Omit<
  InputOptions,
  | keyof OverwriteInputOptionsForCli
  | 'input'
  | 'plugins'
  | 'onwarn'
  | 'onLog'
  | 'resolve'
  | 'experimental'
  | 'watch'
> &
  OverwriteInputOptionsForCli;
