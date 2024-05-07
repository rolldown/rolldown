import { Plugin, ParallelPlugin } from '../plugin'
import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'
import { LogLevelOptionSchema } from '../log/logging'

const inputOptionsSchema = z.strictObject({
  input: z.string().or(z.string().array()).or(z.record(z.string())).optional(),
  plugins: zodExt.phantom<Plugin | ParallelPlugin>().array().optional(),
  external: zodExt
    .stringOrRegExp()
    .or(zodExt.stringOrRegExp().array())
    .or(
      z
        .function()
        .args(z.string(), z.string().optional(), z.boolean())
        .returns(zodExt.voidNullableWith(z.boolean()))
        .optional(),
    )
    .optional(),
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
  logLevel: LogLevelOptionSchema.optional(),
})

export type InputOptions = z.infer<typeof inputOptionsSchema>
