import { format } from 'path'
import { BindingOutputOptions, RenderedChunk } from '../binding'
import { unimplemented } from '../utils'
import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'

const addonFunctionSchema = z
  .function()
  .args(zodExt.phantom<RenderedChunk>())
  .returns(z.string().or(z.promise(z.string())))

const outputOptionsSchema = z.strictObject({
  dir: z.string().optional(),
  exports: z.literal('named').optional(),
  format: z
    .literal('es')
    .or(z.literal('cjs'))
    .or(z.literal('es'))
    .or(z.literal('esm'))
    .or(z.literal('module'))
    .or(z.literal('commonjs'))
    .optional(),
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
  entryFileNames: z.string().optional(),
  chunkFileNames: z.string().optional(),
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

type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>
