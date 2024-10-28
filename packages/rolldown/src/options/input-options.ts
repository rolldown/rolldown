import type { RolldownPluginRec } from '../plugin'
import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'
import {
  LogLevelOptionSchema,
  LogLevelSchema,
  LogLevelWithErrorSchema,
  RollupLogSchema,
  RollupLogWithStringSchema,
} from '../log/logging'
import { TreeshakingOptions } from '../treeshake'
import { underline, gray, yellow, dim } from '../cli/colors'

const inputOptionSchema = z
  .string()
  .or(z.string().array())
  .or(z.record(z.string()))

const externalSchema = zodExt
  .stringOrRegExp()
  .or(zodExt.stringOrRegExp().array())
  .or(
    z
      .function()
      .args(z.string(), z.string().optional(), z.boolean())
      .returns(zodExt.voidNullableWith(z.boolean())),
  )

const moduleTypesSchema = z.record(
  z
    .literal('js')
    .or(z.literal('jsx'))
    .or(z.literal('ts'))
    .or(z.literal('tsx'))
    .or(z.literal('json'))
    .or(z.literal('text'))
    .or(z.literal('base64'))
    .or(z.literal('dataurl'))
    .or(z.literal('binary'))
    .or(z.literal('empty'))
    .or(z.literal('css')),
)

const jsxOptionsSchema = z.strictObject({
  mode: z
    .literal('classic')
    .or(z.literal('automatic'))
    .describe('Jsx transformation mode')
    .optional(), // The rollup preserve is not supported at now
  factory: z.string().describe('Jsx element transformation').optional(),
  fragment: z.string().describe('Jsx fragment transformation').optional(),
  importSource: z
    .string()
    .describe('Import the factory of element and fragment if mode is classic')
    .optional(),
  jsxImportSource: z
    .string()
    .describe('Import the factory of element and fragment if mode is automatic')
    .optional(),
  refresh: z.boolean().describe('React refresh transformation').optional(),
  development: z
    .boolean()
    .describe('Development specific information')
    .optional(),
  // The rollup preset is not supported at now
})

const stringOrRegExpSchema = zodExt
  .stringOrRegExp()
  .or(zodExt.stringOrRegExp().array())

const watchOptionsSchema = z.strictObject({
  skipWrite: z.boolean().describe('Skip the bundle.write() step').optional(),
  notify: z
    .strictObject({
      pollInterval: z.number().optional(),
      compareContents: z.boolean().optional(),
    })
    .describe('Notify options')
    .optional(),
  include: stringOrRegExpSchema.optional(),
  exclude: stringOrRegExpSchema.optional(),
  chokidar: z.any().optional(),
})

export const inputOptionsSchema = z.strictObject({
  input: inputOptionSchema.optional(),
  plugins: zodExt.phantom<RolldownPluginRec>().array().optional(),
  external: externalSchema.optional(),
  resolve: z
    .strictObject({
      alias: z.record(z.string()).optional(),
      aliasFields: z.array(z.array(z.string())).optional(),
      conditionNames: zodExt.optionalStringArray(),
      extensionAlias: z.record(z.string(), z.array(z.string())).optional(),
      exportsFields: z.array(z.array(z.string())).optional(),
      extensions: zodExt.optionalStringArray(),
      mainFields: zodExt.optionalStringArray(),
      mainFiles: zodExt.optionalStringArray(),
      modules: zodExt.optionalStringArray(),
      symlinks: z.boolean().optional(),
      tsconfigFilename: z.string().optional(),
    })
    .optional(),
  cwd: z.string().describe('current working directory.').optional(),
  platform: z
    .literal('node')
    .or(z.literal('browser'))
    .or(z.literal('neutral'))
    .describe(
      `platform for which the code should be generated (node, ${underline('browser')}, neutral).`,
    )
    .optional(),
  shimMissingExports: z.boolean().optional(),
  // FIXME: should use a more specific schema
  treeshake: zodExt.phantom<boolean | TreeshakingOptions>().optional(),
  logLevel: LogLevelOptionSchema.describe(
    `log level (${dim('silent')}, ${underline(gray('info'))}, debug, ${yellow('warn')})`,
  ).optional(),
  onLog: z
    .function()
    .args(
      LogLevelSchema,
      RollupLogSchema,
      z.function().args(LogLevelWithErrorSchema, RollupLogWithStringSchema),
    )
    .optional(),
  onwarn: z
    .function()
    .args(
      RollupLogSchema,
      z
        .function()
        .args(
          RollupLogWithStringSchema.or(
            z.function().returns(RollupLogWithStringSchema),
          ),
        ),
    )
    .optional(),
  moduleTypes: moduleTypesSchema
    .describe('module types for customized extensions.')
    .optional(),
  experimental: z
    .strictObject({
      enableComposingJsPlugins: z.boolean().optional(),
      strictExecutionOrder: z.boolean().optional(),
      disableLiveBindings: z.boolean().optional(),
    })
    .optional(),
  define: z.record(z.string()).describe('define global variables').optional(),
  inject: z.record(z.string().or(z.tuple([z.string(), z.string()]))).optional(),
  profilerNames: z.boolean().optional(),
  jsx: jsxOptionsSchema.optional(),
  watch: watchOptionsSchema.or(z.literal(false)).optional(),
})

export const inputCliOptionsSchema = inputOptionsSchema
  .extend({
    external: z
      .array(z.string())
      .describe(
        'Comma-separated list of module ids to exclude from the bundle `<module-id>,...`',
      )
      .optional(),
    inject: z
      .record(z.string())
      .describe('inject import statements on demand')
      .optional(),
    treeshake: z
      .boolean()
      .describe('enable treeshaking')
      .default(true)
      .optional(),
  })
  .omit({
    input: true,
    plugins: true,
    onwarn: true,
    onLog: true,
    resolve: true,
    experimental: true,
    profilerNames: true,
    watch: true,
  })

type RawInputOptions = z.infer<typeof inputOptionsSchema>
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

export type InputOption = z.infer<typeof inputOptionSchema>
export type InputOptions = Omit<
  RawInputOptions,
  keyof OverwriteInputOptionsWithDoc
> &
  OverwriteInputOptionsWithDoc
export type ExternalOption = z.infer<typeof externalSchema>

export type JsxOptions = z.infer<typeof jsxOptionsSchema>
