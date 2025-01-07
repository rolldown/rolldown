import type { RolldownPluginOption } from '../plugin'
import type {
  LogLevel,
  LogLevelOption,
  LogLevelWithError,
  RollupLog,
  RollupLogWithString,
} from '../log/logging'
import type { NullValue, StringOrRegExp } from '../types/utils'
import type { TreeshakingOptions } from '../types/module-side-effects'

export type InputOption = string | string[] | Record<string, string>

export type ExternalOption =
  | StringOrRegExp
  | StringOrRegExp[]
  | ((
      id: string,
      parentId: string | undefined,
      isResolved: boolean,
    ) => NullValue<boolean>)

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
>

export interface JsxOptions {
  mode?: 'classic' | 'automatic' | 'preserve'
  factory?: string
  fragment?: string
  importSource?: string
  jsxImportSource?: string
  refresh?: boolean
  development?: boolean
}

export interface WatchOptions {
  skipWrite?: boolean
  notify?: {
    pollInterval?: number
    compareContents?: boolean
  }
  include?: StringOrRegExp | StringOrRegExp[]
  exclude?: StringOrRegExp | StringOrRegExp[]
  chokidar?: any
}

export interface ChecksOptions {
  /**
   * Wether to emit warnings when detecting circular dependencies.
   * @default false
   */
  circularDependency?: boolean
}

export interface InputOptions {
  input?: InputOption
  plugins?: RolldownPluginOption
  external?: ExternalOption
  resolve?: {
    alias?: Record<string, string[] | string>
    aliasFields?: string[][]
    conditionNames?: string[]
    /**
     * Map of extensions to alternative extensions.
     *
     * With writing `import './foo.js'` in a file, you want to resolve it to `foo.ts` instead of `foo.js`.
     * You can achieve this by setting: `extensionAlias: { '.js': ['.ts', '.js'] }`.
     */
    extensionAlias?: Record<string, string[]>
    exportsFields?: string[][]
    extensions?: string[]
    mainFields?: string[]
    mainFiles?: string[]
    modules?: string[]
    symlinks?: boolean
    tsconfigFilename?: string
  }
  cwd?: string
  /**
   * Expected platform where the code run.
   *
   * @default
   * - 'node' if the format is 'cjs'
   * - 'browser' for other formats
   */
  platform?: 'node' | 'browser' | 'neutral'
  shimMissingExports?: boolean
  treeshake?: boolean | TreeshakingOptions
  logLevel?: LogLevelOption
  onLog?: (
    level: LogLevel,
    log: RollupLog,
    defaultHandler: (
      level: LogLevelWithError,
      log: RollupLogWithString,
    ) => void,
  ) => void
  onwarn?: (
    warning: RollupLog,
    defaultHandler: (
      warning: RollupLogWithString | (() => RollupLogWithString),
    ) => void,
  ) => void
  moduleTypes?: ModuleTypes
  experimental?: {
    enableComposingJsPlugins?: boolean
    strictExecutionOrder?: boolean
    disableLiveBindings?: boolean
    viteMode?: boolean
    resolveNewUrlToAsset?: boolean
  }
  define?: Record<string, string>
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
  inject?: Record<string, string | [string, string]>
  profilerNames?: boolean
  /**
   * JSX options.
   * The `false` is disabled jsx parser, it will give you a syntax error if you use jsx syntax
   * The `mode: preserve` is disabled jsx transformer, it perverse original jsx syntax in the output.
   * The `mode: classic` is enabled jsx `classic` transformer.
   * The `mode: automatic` is enabled jsx `automatic` transformer.
   * @default mode = 'automatic'
   */
  jsx?: false | JsxOptions
  watch?: WatchOptions | false
  dropLabels?: string[]
  keepNames?: boolean
  checks?: ChecksOptions
}

interface OverwriteInputOptionsForCli {
  external?: string[]
  inject?: Record<string, string>
  treeshake?: boolean
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
  | 'profilerNames'
  | 'watch'
> &
  OverwriteInputOptionsForCli
