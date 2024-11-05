import { RolldownPluginRec } from '../plugin'
import {
  LogLevel,
  LogLevelOption,
  LogLevelWithError,
  RollupLog,
  RollupLogWithString,
} from '../log/logging'
import { TreeshakingOptions } from '../treeshake'
import { NullValue, StringOrRegExp } from './utils'

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
  mode?: 'classic' | 'automatic'
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

export interface RawInputOptions {
  input?: InputOption
  plugins?: RolldownPluginRec[]
  external?: ExternalOption
  resolve?: {
    alias?: Record<string, string>
    aliasFields?: string[][]
    conditionNames?: string[]
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
  }
  define?: Record<string, string>
  inject?: Record<string, string | [string, string]>
  profilerNames?: boolean
  jsx?: JsxOptions
  watch?: WatchOptions | false
}

export type InputCliOptions = Omit<
  RawInputOptions,
  | 'input'
  | 'plugins'
  | 'onwarn'
  | 'onLog'
  | 'resolve'
  | 'experimental'
  | 'profilerNames'
  | 'watch'
  | 'external'
  | 'inject'
  | 'treeshake'
> & {
  external?: string[]
  inject?: Record<string, string>
  treeshake?: boolean
}

interface OverwriteInputOptionsWithDoc {
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
  inject?: RawInputOptions['inject']
}

export type InputOptions = Omit<
  RawInputOptions,
  keyof OverwriteInputOptionsWithDoc
> &
  OverwriteInputOptionsWithDoc
