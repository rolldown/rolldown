import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'
import { underline, gray, yellow, dim } from '../cli/colors'
import {
  LogLevelOptionSchema,
  LogLevelSchema,
  LogLevelWithErrorSchema,
  RollupLogSchema,
  RollupLogWithStringSchema,
} from '../log/logging'
import type { RolldownPluginRec } from '../plugin'
import { TreeshakingOptionsSchema } from '../treeshake'
import type {
  ExternalOption,
  InputCliOptions,
  InputOption,
  JsxOptions,
  ModuleTypes,
  InputOptions,
  WatchOptions,
} from '../types/input-options'
import type { StringOrRegExp } from '../types/utils'

const inputOptionSchema = z
  .string()
  .or(z.string().array())
  .or(z.record(z.string())) satisfies z.ZodType<InputOption>

const externalSchema = zodExt
  .stringOrRegExp()
  .or(zodExt.stringOrRegExp().array())
  .or(
    z
      .function()
      .args(z.string(), z.string().optional(), z.boolean())
      .returns(zodExt.voidNullableWith(z.boolean())),
  ) satisfies z.ZodType<ExternalOption>

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
) satisfies z.ZodType<ModuleTypes>

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
}) satisfies z.ZodType<JsxOptions>

const stringOrRegExpSchema = zodExt
  .stringOrRegExp()
  .or(zodExt.stringOrRegExp().array()) satisfies z.ZodType<
  StringOrRegExp | StringOrRegExp[]
>

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
}) satisfies z.ZodType<WatchOptions>

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
  cwd: z.string().describe('Current working directory').optional(),
  platform: z
    .literal('node')
    .or(z.literal('browser'))
    .or(z.literal('neutral'))
    .describe(
      `Platform for which the code should be generated (node, ${underline('browser')}, neutral)`,
    )
    .optional(),
  shimMissingExports: z
    .boolean()
    .describe(`Create shim variables for missing exports`)
    .optional(),
  treeshake: TreeshakingOptionsSchema.optional(),
  logLevel: LogLevelOptionSchema.describe(
    `Log level (${dim('silent')}, ${underline(gray('info'))}, debug, ${yellow('warn')})`,
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
    .describe('Module types for customized extensions')
    .optional(),
  experimental: z
    .strictObject({
      enableComposingJsPlugins: z.boolean().optional(),
      strictExecutionOrder: z.boolean().optional(),
      disableLiveBindings: z.boolean().optional(),
    })
    .optional(),
  define: z.record(z.string()).describe('Define global variables').optional(),
  inject: z.record(z.string().or(z.tuple([z.string(), z.string()]))).optional(),
  profilerNames: z.boolean().optional(),
  jsx: jsxOptionsSchema.optional(),
  watch: watchOptionsSchema.or(z.literal(false)).optional(),
  dropLabels: z
    .array(z.string())
    .describe('Remove labeled statements with these label names')
    .optional(),
}) satisfies z.ZodType<InputOptions>

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
      .describe('Inject import statements on demand')
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
  }) satisfies z.ZodType<InputCliOptions>
