import type { TransformOptions } from '../binding';
import type {
  LogLevel,
  LogLevelOption,
  LogOrStringHandler,
  RollupLog,
  RollupLogWithString,
} from '../log/logging';
import type { RolldownPluginOption } from '../plugin';
import type { TreeshakingOptions } from '../types/module-side-effects';
import type { NullValue, StringOrRegExp } from '../types/utils';
import type { ChecksOptions } from './generated/checks-options';

export type InputOption = string | string[] | Record<string, string>;

// Omit those key that are part of rolldown option
type OxcTransformOption = Omit<
  TransformOptions,
  | 'sourceType'
  | 'lang'
  | 'cwd'
  | 'sourcemap'
  | 'define'
  | 'inject'
>;

export type ExternalOption =
  | StringOrRegExp
  | StringOrRegExp[]
  | ((
    id: string,
    parentId: string | undefined,
    isResolved: boolean,
  ) => NullValue<boolean>);

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
  skipWrite?: boolean;
  buildDelay?: number;
  notify?: {
    pollInterval?: number;
    compareContents?: boolean;
  };
  include?: StringOrRegExp | StringOrRegExp[];
  exclude?: StringOrRegExp | StringOrRegExp[];
  onInvalidate?: (id: string) => void;
  clearScreen?: boolean;
}

type MakeAbsoluteExternalsRelative = boolean | 'ifRelativeSource';

export type HmrOptions = boolean | {
  host?: string;
  port?: number;
  implement?: string;
};

export type OptimizationOptions = {
  /**
   * Inline imported constant values during bundling instead of preserving variable references.
   *
   * When enabled, constant values from imported modules will be inlined at their usage sites,
   * potentially reducing bundle size and improving runtime performance by eliminating variable lookups.
   *
   * **example**
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
  inlineConst?: boolean;
};

export type AttachDebugOptions = 'none' | 'simple' | 'full';

type ChunkModulesOrder = 'exec-order' | 'module-id';

interface RollupJsxOptions {
  mode?: 'classic' | 'automatic' | 'preserve';
  factory?: string;
  fragment?: string;
  importSource?: string;
  jsxImportSource?: string;
}

export interface InputOptions {
  input?: InputOption;
  plugins?: RolldownPluginOption;
  external?: ExternalOption;
  resolve?: {
    /**
     * > [!WARNING]
     * > `resolve.alias` will not call `resolveId` hooks of other plugin.
     * > If you want to call `resolveId` hooks of other plugin, use `aliasPlugin` from `rolldown/experimental` instead.
     * > You could find more discussion in [this issue](https://github.com/rolldown/rolldown/issues/3615)
     */
    alias?: Record<string, string[] | string>;
    aliasFields?: string[][];
    conditionNames?: string[];
    /**
     * Map of extensions to alternative extensions.
     *
     * With writing `import './foo.js'` in a file, you want to resolve it to `foo.ts` instead of `foo.js`.
     * You can achieve this by setting: `extensionAlias: { '.js': ['.ts', '.js'] }`.
     */
    extensionAlias?: Record<string, string[]>;
    exportsFields?: string[][];
    extensions?: string[];
    mainFields?: string[];
    mainFiles?: string[];
    modules?: string[];
    symlinks?: boolean;
  };
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
   * - 'node' if the format is 'cjs'
   * - 'browser' for other formats
   */
  platform?: 'node' | 'browser' | 'neutral';
  shimMissingExports?: boolean;
  treeshake?: boolean | TreeshakingOptions;
  logLevel?: LogLevelOption;
  onLog?: (
    level: LogLevel,
    log: RollupLog,
    defaultHandler: LogOrStringHandler,
  ) => void;
  onwarn?: (
    warning: RollupLog,
    defaultHandler: (
      warning: RollupLogWithString | (() => RollupLogWithString),
    ) => void,
  ) => void;
  moduleTypes?: ModuleTypes;
  experimental?: {
    strictExecutionOrder?: boolean;
    disableLiveBindings?: boolean;
    viteMode?: boolean;
    resolveNewUrlToAsset?: boolean;
    hmr?: HmrOptions;
    /**
     * Control which order should use when rendering modules in chunk
     *
     * - Type: `'exec-order' | 'module-id'
     * - Default: `'exec-order'`
     *
     * - `exec-order`: Almost equivalent to the topological order of the module graph, but specially handling when module graph has cycle.
     * - `module-id`: This is more friendly for gzip compression, especially for some javascript static asset lib (e.g. icon library)
     * > [!NOTE]
     * > Try to sort the modules by their module id if possible(Since rolldown scope hoist all modules in the chunk, we only try to sort those modules by module id if we could ensure runtime behavior is correct after sorting).
     */
    chunkModulesOrder?: ChunkModulesOrder;
    /**
     * Attach debug information to the output bundle.
     *
     * - Type: `'none' | 'simple' | 'full'`
     * - Default: `'simple'`
     *
     * - `none`: No debug information is attached.
     * - `simple`: Attach comments indicating which files the bundled code comes from. These comments could be removed by the minifier.
     * - `full`: Attach detailed debug information to the output bundle. These comments are using legal comment syntax, so they won't be removed by the minifier.
     *
     * > [!WARNING]
     * > You shouldn't use `full` in the production build.
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
     * Example configuration snippet:
     *
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
     * > [!NOTE]
     * > If you want to learn more, you can check out the example here: [examples/chunk-import-map](https://github.com/rolldown/rolldown/tree/main/examples/chunk-import-map)
     */
    chunkImportMap?: boolean | { baseUrl?: string; fileName?: string };
    onDemandWrapping?: boolean;
    /**
     * Required to be used with `watch` mode.
     */
    incrementalBuild?: boolean;
    transformHiresSourcemap?: boolean | 'boundary';
  };
  /**
   * Replace global variables or [property accessors](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Property_accessors) with the provided values.
   *
   * # Examples
   *
   * - Replace the global variable `IS_PROD` with `true`
   *
   * ```js rolldown.config.js
   * export default defineConfig({ define: { IS_PROD: 'true' // or JSON.stringify(true) } })
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
   * export default defineConfig({ define: { 'process.env.NODE_ENV': "'production'" } })
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
  profilerNames?: boolean;
  /**
   * @deprecated Use {@link OxcTransformOption.jsx} instead.
   *
   * This top-level `jsx` option will be removed in a future release.
   * It is only kept for backward compatibility and will be mapped internally to `transform.jsx`.
   *
   * - `false` disables the JSX parser, resulting in a syntax error if JSX syntax is used.
   * - `"preserve"` disables the JSX transformer, preserving the original JSX syntax in the output.
   * - `"react"` enables the `classic` JSX transformer.
   * - `"react-jsx"` enables the `automatic` JSX transformer.
   *
   * @default runtime = "automatic"
   */
  jsx?: false | 'react' | 'react-jsx' | 'preserve' | RollupJsxOptions;
  /**
   * Configure how the code is transformed. This process happens after the `transform` hook.
   *
   * To transpile [legacy decorators](https://github.com/tc39/proposal-decorators/tree/4ac0f4cd31bd0f2e8170cb4c5136e51671e46c8d), you could use
   *
   * ```js
   * export default defineConfig({
   *   transform: {
   *     decorator: {
   *       legacy: true,
   *     },
   *   },
   * })
   * ```
   *
   * For latest decorators proposal, rolldown is able to bundle them but doesn't support transpiling them yet.
   */
  transform?: OxcTransformOption;
  watch?: WatcherOptions | false;
  dropLabels?: string[];
  keepNames?: boolean;
  checks?: ChecksOptions;
  makeAbsoluteExternalsRelative?: MakeAbsoluteExternalsRelative;
  debug?: {
    sessionId?: string;
  };
  preserveEntrySignatures?:
    | false
    | 'strict'
    | 'allow-extension'
    | 'exports-only';
  optimization?: OptimizationOptions;
  context?: string;
  /**
   * Allows you to specify where to find the TypeScript configuration file.
   *
   * You may provide:
   * - a relative path to the configuration file. It will be resolved relative to cwd.
   * - an absolute path to the configuration file.
   *
   * When a tsconfig path is specified, the module resolver will respect `compilerOptions.paths` from the specified `tsconfig.json`,
   * and the tsconfig options will be merged with the top-level `transform` options, with the `transform` options taking precedence.
   */
  tsconfig?: string;
}

interface OverwriteInputOptionsForCli {
  external?: string[];
  inject?: Record<string, string>;
  treeshake?: boolean;
}

export type InputCliOptions =
  & Omit<
    InputOptions,
    | keyof OverwriteInputOptionsForCli
    | 'input'
    | 'plugins'
    | 'onwarn'
    | 'onLog'
    | 'resolve'
    | 'experimental'
    | 'profilerNames'
    | 'watch'
  >
  & OverwriteInputOptionsForCli;
