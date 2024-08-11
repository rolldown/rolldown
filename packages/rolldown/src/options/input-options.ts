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
import { TreeshakingOptionsSchema, TreeshakingOptions } from '../treeshake'

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

const inputOptionsSchema = z.strictObject({
  input: inputOptionSchema.optional(),
  plugins: zodExt.phantom<RolldownPluginRec>().array().optional(),
  external: externalSchema.optional(),
  resolve: z
    .strictObject({
      alias: z.record(z.string()).optional(),
      aliasFields: z.array(z.array(z.string())).optional(),
      conditionNames: zodExt.optionalStringArray(),
      exportsFields: z.array(z.array(z.string())).optional(),
      extensions: zodExt.optionalStringArray(),
      mainFields: zodExt.optionalStringArray(),
      mainFiles: zodExt.optionalStringArray(),
      modules: zodExt.optionalStringArray(),
      symlinks: z.boolean().optional(),
      tsconfigFilename: z.string().optional(),
    })
    .optional(),
  cwd: z.string().optional(),
  platform: z
    .literal('node')
    .or(z.literal('browser'))
    .or(z.literal('neutral'))
    .optional(),
  shimMissingExports: z.boolean().optional(),
  treeshake: z.boolean().or(TreeshakingOptionsSchema).optional(),
  logLevel: LogLevelOptionSchema.optional(),
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
  moduleTypes: z
    .record(
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
        .or(z.literal('empty')),
    )
    .optional(),
  experimental: z
    .strictObject({
      enableComposingJsPlugins: z.boolean().optional(),
    })
    .optional(),
  define: z.record(z.string()).optional(),
  inject: z.record(z.string().or(z.tuple([z.string(), z.string()]))).optional(),
})

export type InputOption = z.infer<typeof inputOptionSchema>
export type ExternalOption = z.infer<typeof externalSchema>
export type InputOptions = Omit<
  z.infer<typeof inputOptionsSchema>,
  'treeshake'
> & {
  treeshake?: boolean | TreeshakingOptions
}
