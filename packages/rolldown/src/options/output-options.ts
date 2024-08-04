import type { RenderedChunk } from '../binding'
import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'

const ModuleFormatSchema = z
  .literal('es')
  .or(z.literal('cjs'))
  .or(z.literal('esm'))
  .or(z.literal('module'))
  .or(z.literal('commonjs'))
  .or(z.literal('iife'))
  .optional()

const addonFunctionSchema = z
  .function()
  .args(zodExt.phantom<RenderedChunk>())
  .returns(z.string().or(z.promise(z.string())))

const outputOptionsSchema = z.strictObject({
  dir: z.string().optional(),
  exports: z
    .literal('auto')
    .or(z.literal('named'))
    .or(z.literal('default'))
    .or(z.literal('none'))
    .optional(),
  format: ModuleFormatSchema,
  sourcemap: z
    .boolean()
    .or(z.literal('inline'))
    .or(z.literal('hidden'))
    .optional(),
  sourcemapIgnoreList: z
    .boolean()
    .or(zodExt.phantom<SourcemapIgnoreListOption>())
    .optional(),
  sourcemapPathTransform: zodExt
    .phantom<SourcemapPathTransformOption>()
    .optional(),
  banner: z.string().or(addonFunctionSchema).optional(),
  footer: z.string().or(addonFunctionSchema).optional(),
  intro: z.string().or(addonFunctionSchema).optional(),
  outro: z.string().or(addonFunctionSchema).optional(),
  esModule: z
    .literal('if-default-prop')
    .or(z.literal('always'))
    .or(z.literal('never'))
    .or(z.boolean())
    .optional(),
  extend: z.boolean().optional(),
  entryFileNames: z.string().optional(),
  chunkFileNames: z.string().optional(),
  assetFileNames: z.string().optional(),
  minify: z.boolean().optional(),
  name: z.string().optional(),
  globals: z.record(z.string()).optional(),
})

export type OutputOptions = z.infer<typeof outputOptionsSchema>

export type SourcemapIgnoreListOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => boolean

export type SourcemapPathTransformOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => string

export type ModuleFormat = z.infer<typeof ModuleFormatSchema>
