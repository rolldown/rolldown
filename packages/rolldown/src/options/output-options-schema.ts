import { z } from 'zod'
import * as zodExt from '../utils/zod-ext'
import { bold, underline } from '../cli/colors'
import type { RenderedChunk, PreRenderedChunk } from '../binding'
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../rollup'
import type {
  AddonFunction,
  ChunkFileNamesFunction,
  GlobalsFunction,
  ModuleFormat,
  OutputCliOptions,
  OutputOptions,
} from '../options/output-options'

const ModuleFormatSchema = z
  .literal('es')
  .or(z.literal('cjs'))
  .or(z.literal('esm'))
  .or(z.literal('module'))
  .or(z.literal('commonjs'))
  .or(z.literal('iife'))
  .or(z.literal('umd'))
  .describe(
    `Output format of the generated bundle (supports ${underline('esm')}, cjs, and iife)`,
  ) satisfies z.ZodType<ModuleFormat>

const addonFunctionSchema = z
  .function()
  .args(zodExt.phantom<RenderedChunk>())
  .returns(
    z.string().or(z.promise(z.string())),
  ) satisfies z.ZodType<AddonFunction>

const chunkFileNamesFunctionSchema = z
  .function()
  .args(zodExt.phantom<PreRenderedChunk>())
  .returns(z.string()) satisfies z.ZodType<ChunkFileNamesFunction>

const GlobalsFunctionSchema = z
  .function()
  .args(z.string())
  .returns(z.string()) satisfies z.ZodType<GlobalsFunction>

const outputOptionsSchema = z.strictObject({
  dir: z
    .string()
    .describe('Output directory, defaults to `dist` if `file` is not set')
    .optional(),
  file: z.string().describe('Single output file').optional(),
  exports: z
    .literal('auto')
    .or(z.literal('named'))
    .or(z.literal('default'))
    .or(z.literal('none'))
    .describe(
      `Specify a export mode (${underline('auto')}, named, default, none)`,
    )
    .optional(),
  hashCharacters: z
    .literal('base64')
    .or(z.literal('base36'))
    .or(z.literal('hex'))
    .describe('Use the specified character set for file hashes')
    .optional(),
  format: ModuleFormatSchema.optional(),
  sourcemap: z
    .boolean()
    .or(z.literal('inline'))
    .or(z.literal('hidden'))
    .describe(
      `Generate sourcemap (\`-s inline\` for inline, or ${bold('pass the `-s` on the last argument if you want to generate `.map` file')})`,
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
    .describe('Extend global variable defined by name in IIFE / UMD formats')
    .optional(),
  esModule: z.literal('if-default-prop').or(z.boolean()).optional(),
  assetFileNames: z
    .string()
    .describe('Name pattern for asset files')
    .optional(),
  entryFileNames: z
    .string()
    .or(chunkFileNamesFunctionSchema)
    .describe('Name pattern for emitted entry chunks')
    .optional(),
  chunkFileNames: z
    .string()
    .or(chunkFileNamesFunctionSchema)
    .describe('Name pattern for emitted secondary chunks')
    .optional(),
  cssEntryFileNames: z
    .string()
    .or(chunkFileNamesFunctionSchema)
    .describe('Name pattern for emitted css entry chunks')
    .optional(),
  cssChunkFileNames: z
    .string()
    .or(chunkFileNamesFunctionSchema)
    .describe('Name pattern for emitted css secondary chunks')
    .optional(),
  minify: z.boolean().describe('Minify the bundled file.').optional(),
  name: z.string().describe('Name for UMD / IIFE format outputs').optional(),
  globals: z
    .record(z.string())
    .or(GlobalsFunctionSchema)
    .describe(
      'Global variable of UMD / IIFE dependencies (syntax: `key=value`)',
    )
    .optional(),
  externalLiveBindings: z
    .boolean()
    .describe('external live bindings')
    .default(true)
    .optional(),
  inlineDynamicImports: z
    .boolean()
    .describe('Inline dynamic imports')
    .default(false)
    .optional(),
  advancedChunks: z
    .strictObject({
      minSize: z.number().optional(),
      minShareCount: z.number().optional(),
      groups: z
        .array(
          z.strictObject({
            name: z.string(),
            test: z.string().or(z.instanceof(RegExp)).optional(),
            priority: z.number().optional(),
            minSize: z.number().optional(),
            minShareCount: z.number().optional(),
          }),
        )
        .optional(),
    })
    .optional(),
  comments: z
    .enum(['none', 'preserve-legal'])
    .describe('Control comments in the output')
    .optional(),
}) satisfies z.ZodType<OutputOptions>

const getAddonDescription = (
  placement: 'bottom' | 'top',
  wrapper: 'inside' | 'outside',
) => {
  return `Code to insert the ${bold(placement)} of the bundled file (${bold(wrapper)} the wrapper function)`
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
        'Always generate `__esModule` marks in non-ESM formats, defaults to `if-default-prop` (use `--no-esModule` to always disable)',
      )
      .optional(),
    globals: z
      .record(z.string())
      .describe(
        'Global variable of UMD / IIFE dependencies (syntax: `key=value`)',
      )
      .optional(),
    advancedChunks: z
      .strictObject({
        minSize: z.number().describe('Minimum size of the chunk').optional(),
        minShareCount: z
          .number()
          .describe('Minimum share count of the chunk')
          .optional(),
      })
      .optional(),
  })
  .omit({
    sourcemapPathTransform: true,
    sourcemapIgnoreList: true,
  }) satisfies z.ZodType<OutputCliOptions>
