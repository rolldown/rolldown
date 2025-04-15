import { TransformOptions } from '../binding';
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
// Note: `target` should be omit either because it is also used in `minifier`
type OxcTransformOption = Omit<
  TransformOptions,
  | 'sourceType'
  | 'lang'
  | 'cwd'
  | 'sourcemap'
  | 'jsx'
  | 'define'
  | 'inject'
  | 'target'
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
>;

export interface JsxOptions {
  mode?: 'classic' | 'automatic' | 'preserve';
  factory?: string;
  fragment?: string;
  importSource?: string;
  jsxImportSource?: string;
  refresh?: boolean;
  development?: boolean;
}

export interface WatchOptions {
  skipWrite?: boolean;
  buildDelay?: number;
  notify?: {
    pollInterval?: number;
    compareContents?: boolean;
  };
  include?: StringOrRegExp | StringOrRegExp[];
  exclude?: StringOrRegExp | StringOrRegExp[];
}

export type MakeAbsoluteExternalsRelative = boolean | 'ifRelativeSource';

export type HmrOptions = boolean | {
  host?: string;
  port?: number;
  implement?: string;
};

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
    tsconfigFilename?: string;
  };
  cwd?: string;
  /**
   * Expected platform where the code run.
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
    enableComposingJsPlugins?: boolean;
    strictExecutionOrder?: boolean;
    disableLiveBindings?: boolean;
    viteMode?: boolean;
    resolveNewUrlToAsset?: boolean;
    hmr?: HmrOptions;
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
   * - `false` disables the JSX parser, resulting in a syntax error if JSX syntax is used.
   * - `"preserve"` disables the JSX transformer, preserving the original JSX syntax in the output.
   * - `"react"` enables the `classic` JSX transformer.
   * - `"react-jsx"` enables the `automatic` JSX transformer.
   *
   * @default mode = "automatic"
   */
  jsx?: false | 'react' | 'react-jsx' | 'preserve' | JsxOptions;
  transform?: OxcTransformOption;
  watch?: WatchOptions | false;
  dropLabels?: string[];
  keepNames?: boolean;
  checks?: ChecksOptions;
  makeAbsoluteExternalsRelative?: MakeAbsoluteExternalsRelative;
  debug?: {
    buildId?: string;
  };
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
