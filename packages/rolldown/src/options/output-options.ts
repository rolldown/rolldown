import type { PreRenderedChunk, RenderedChunk } from '../binding'
import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'
import { bold, underline } from '../cli/colors'

const ModuleFormatSchema = z
  .literal('es')
  .or(z.literal('cjs'))
  .or(z.literal('esm'))
  .or(z.literal('module'))
  .or(z.literal('commonjs'))
  .or(z.literal('iife'))
  .describe(
    `output format of the generated bundle (supports ${underline('esm')}, cjs, and iife).`,
  )
  .optional()

const addonFunctionSchema = z
  .function()
  .args(zodExt.phantom<RenderedChunk>())
  .returns(z.string().or(z.promise(z.string())))

const chunkFileNamesFunctionSchema = z
  .function()
  .args(zodExt.phantom<PreRenderedChunk>())
  .returns(z.string())

const outputOptionsSchema = z.strictObject({
  dir: z.string().describe('Output directory, defaults to `dist`.').optional(),
  exports: z
    .literal('auto')
    .or(z.literal('named'))
    .or(z.literal('default'))
    .or(z.literal('none'))
    .describe(
      `specify a export mode (${underline('auto')}, named, default, none)`,
    )
    .optional(),
  format: ModuleFormatSchema,
  sourcemap: z
    .boolean()
    .or(z.literal('inline'))
    .or(z.literal('hidden'))
    .describe(
      `generate sourcemap (\`-s inline\` for inline, or ${bold('pass the `-s` on the last argument if you want to generate `.map` file')}).`,
    )
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
  extend: z
    .boolean()
    .describe('extend global variable defined by name in IIFE or UMD formats')
    .optional(),
  esModule: z.literal('if-default-prop').or(z.boolean()).optional(),
  entryFileNames: z.string().or(chunkFileNamesFunctionSchema).optional(),
  chunkFileNames: z.string().or(chunkFileNamesFunctionSchema).optional(),
  assetFileNames: z.string().optional(),
  minify: z.boolean().describe('minify the bundled file.').optional(),
  name: z.string().describe('name for UMD / IIFE format outputs').optional(),
  globals: z
    .record(z.string())
    .describe(
      'Comma-separated list of `module-id:global` pairs (`<module-id>:<global>,...`)',
    )
    .optional(),
  externalLiveBindings: z
    .boolean()
    .describe('use external live bindings')
    .default(true)
    .optional(),
  inlineDynamicImports: z
    .boolean()
    .describe('inline dynamic imports')
    .default(false)
    .optional(),
})

const getAddonDescription = (
  placement: 'bottom' | 'top',
  wrapper: 'inside' | 'outside',
) => {
  return `code to insert the ${bold(placement)} of the bundled file (${bold(wrapper)} the wrapper function).`
}

export const outputCliOptionsSchema = outputOptionsSchema
  .extend({
    // Reject all functions in CLI
    banner: z
      .string()
      .describe(getAddonDescription('top', 'outside'))
      .optional(),
    footer: z
      .string()
      .describe(getAddonDescription('bottom', 'outside'))
      .optional(),
    intro: z.string().describe(getAddonDescription('top', 'inside')).optional(),
    outro: z
      .string()
      .describe(getAddonDescription('bottom', 'inside'))
      .optional(),
    // It is hard to handle the union type in json schema, so use this first.
    esModule: z
      .boolean()
      .describe(
        'always generate `__esModule` marks in non-ESM formats, defaults to `if-default-prop` (use `--no-esModule` to always disable).',
      )
      .optional(),
    sourcemapIgnoreList: z.boolean().optional(),
    sourcemapPathTransform: z.undefined().optional(),
  })
  .omit({ sourcemapPathTransform: true })

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
