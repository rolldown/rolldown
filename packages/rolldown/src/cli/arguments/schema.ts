import { inputCliOptionsSchema } from '../../options/input-options-schema'
import { InputCliOptions } from '../../options/input-options'
import { OutputCliOptions } from '../../options/output-options'
import { outputCliOptionsSchema } from '../../options/output-options-schema'
import type { ObjectSchema } from './types'
import type Z from 'zod'
import { z } from 'zod'

export interface CliOptions extends InputCliOptions, OutputCliOptions {
  config?: string | boolean
  help?: boolean
  version?: boolean
  watch?: boolean
}

export const cliOptionsSchema: Z.ZodType<CliOptions> = z
  .strictObject({
    config: z
      .string()
      .or(z.boolean())
      .describe('Path to the config file (default: `rolldown.config.js`)')
      .optional(),
    help: z.boolean().describe('Show help').optional(),
    version: z.boolean().describe('Show version number').optional(),
    watch: z
      .boolean()
      .describe('Watch files in bundle and rebuild on changes')
      .optional(),
  })
  .merge(inputCliOptionsSchema as Z.AnyZodObject)
  .merge(outputCliOptionsSchema as Z.AnyZodObject) as any // We already explicitly defined the type of `cliOptionsSchema` as `Z.ZodType<CliOptions>`, so we can safely cast it to `any` here.

// TODO: It will be resolved after migrating to `valibot`
// export const schema = zodToJsonSchema(
//   cliOptionsSchema,
// ) as unknown as ObjectSchema

export const schema = {
  type: 'object',
  properties: {
    config: {
      type: ['string', 'boolean'],
      description: 'Path to the config file (default: `rolldown.config.js`)',
    },
    help: { type: 'boolean', description: 'Show help' },
    version: { type: 'boolean', description: 'Show version number' },
    watch: {
      type: 'boolean',
      description: 'Watch files in bundle and rebuild on changes',
    },
    external: {
      type: 'array',
      items: { type: 'string' },
      description:
        'Comma-separated list of module ids to exclude from the bundle `<module-id>,...`',
    },
    cwd: { type: 'string', description: 'Current working directory' },
    platform: {
      anyOf: [
        { type: 'string', enum: ['node', 'browser'] },
        { type: 'string', const: 'neutral' },
      ],
      description:
        'Platform for which the code should be generated (node, \u001b[4mbrowser\u001b[24m, neutral)',
    },
    shimMissingExports: {
      type: 'boolean',
      description: 'Create shim variables for missing exports',
    },
    treeshake: {
      type: 'boolean',
      description: 'enable treeshaking',
      default: true,
    },
    logLevel: {
      anyOf: [
        {
          anyOf: [
            { type: 'string', enum: ['info', 'debug'] },
            { type: 'string', const: 'warn' },
          ],
        },
        { type: 'string', const: 'silent' },
      ],
      description:
        'Log level (\u001b[2msilent\u001b[22m, \u001b[4m\u001b[90minfo\u001b[39m\u001b[24m, debug, \u001b[33mwarn\u001b[39m)',
    },
    moduleTypes: {
      type: 'object',
      additionalProperties: {
        anyOf: [
          {
            anyOf: [
              {
                anyOf: [
                  {
                    anyOf: [
                      {
                        anyOf: [
                          {
                            anyOf: [
                              {
                                anyOf: [
                                  {
                                    anyOf: [
                                      {
                                        anyOf: [
                                          {
                                            type: 'string',
                                            enum: ['js', 'jsx'],
                                          },
                                          { type: 'string', const: 'ts' },
                                        ],
                                      },
                                      { type: 'string', const: 'tsx' },
                                    ],
                                  },
                                  { type: 'string', const: 'json' },
                                ],
                              },
                              { type: 'string', const: 'text' },
                            ],
                          },
                          { type: 'string', const: 'base64' },
                        ],
                      },
                      { type: 'string', const: 'dataurl' },
                    ],
                  },
                  { type: 'string', const: 'binary' },
                ],
              },
              { type: 'string', const: 'empty' },
            ],
          },
          { type: 'string', const: 'css' },
        ],
      },
      description: 'Module types for customized extensions',
    },
    define: {
      type: 'object',
      additionalProperties: { type: 'string' },
      description: 'Define global variables',
    },
    inject: {
      type: 'object',
      additionalProperties: { type: 'string' },
      description: 'Inject import statements on demand',
    },
    jsx: {
      type: 'object',
      properties: {
        mode: {
          type: 'string',
          enum: ['classic', 'automatic'],
          description: 'Jsx transformation mode',
        },
        factory: { type: 'string', description: 'Jsx element transformation' },
        fragment: {
          type: 'string',
          description: 'Jsx fragment transformation',
        },
        importSource: {
          type: 'string',
          description:
            'Import the factory of element and fragment if mode is classic',
        },
        jsxImportSource: {
          type: 'string',
          description:
            'Import the factory of element and fragment if mode is automatic',
        },
        refresh: {
          type: 'boolean',
          description: 'React refresh transformation',
        },
        development: {
          type: 'boolean',
          description: 'Development specific information',
        },
      },
      additionalProperties: false,
    },
    dropLabels: {
      type: 'array',
      items: { type: 'string' },
      description: 'Remove labeled statements with these label names',
    },
    checks: {
      type: 'object',
      properties: {
        circularDependency: {
          type: 'boolean',
          description:
            'Wether to emit warnings when detecting circular dependencies',
        },
      },
      additionalProperties: false,
    },
    dir: {
      type: 'string',
      description: 'Output directory, defaults to `dist` if `file` is not set',
    },
    file: { type: 'string', description: 'Single output file' },
    exports: {
      anyOf: [
        {
          anyOf: [
            { type: 'string', enum: ['auto', 'named'] },
            { type: 'string', const: 'default' },
          ],
        },
        { type: 'string', const: 'none' },
      ],
      description:
        'Specify a export mode (\u001b[4mauto\u001b[24m, named, default, none)',
    },
    hashCharacters: {
      anyOf: [
        { type: 'string', enum: ['base64', 'base36'] },
        { type: 'string', const: 'hex' },
      ],
      description: 'Use the specified character set for file hashes',
    },
    format: {
      anyOf: [
        {
          anyOf: [
            {
              anyOf: [
                {
                  anyOf: [
                    {
                      anyOf: [
                        { type: 'string', enum: ['es', 'cjs'] },
                        { type: 'string', const: 'esm' },
                      ],
                    },
                    { type: 'string', const: 'module' },
                  ],
                },
                { type: 'string', const: 'commonjs' },
              ],
            },
            { type: 'string', const: 'iife' },
          ],
        },
        { type: 'string', const: 'umd' },
      ],
      description:
        'Output format of the generated bundle (supports \u001b[4mesm\u001b[24m, cjs, and iife)',
    },
    sourcemap: {
      anyOf: [
        { anyOf: [{ type: 'boolean' }, { type: 'string', const: 'inline' }] },
        { type: 'string', const: 'hidden' },
      ],
      description:
        'Generate sourcemap (`-s inline` for inline, or \u001b[1mpass the `-s` on the last argument if you want to generate `.map` file\u001b[22m)',
    },
    banner: {
      type: 'string',
      description:
        'Code to insert the \u001b[1mtop\u001b[22m of the bundled file (\u001b[1moutside\u001b[22m the wrapper function)',
    },
    footer: {
      type: 'string',
      description:
        'Code to insert the \u001b[1mbottom\u001b[22m of the bundled file (\u001b[1moutside\u001b[22m the wrapper function)',
    },
    intro: {
      type: 'string',
      description:
        'Code to insert the \u001b[1mtop\u001b[22m of the bundled file (\u001b[1minside\u001b[22m the wrapper function)',
    },
    outro: {
      type: 'string',
      description:
        'Code to insert the \u001b[1mbottom\u001b[22m of the bundled file (\u001b[1minside\u001b[22m the wrapper function)',
    },
    extend: {
      type: 'boolean',
      description:
        'Extend global variable defined by name in IIFE / UMD formats',
    },
    esModule: {
      type: 'boolean',
      description:
        'Always generate `__esModule` marks in non-ESM formats, defaults to `if-default-prop` (use `--no-esModule` to always disable)',
    },
    assetFileNames: {
      type: 'string',
      description: 'Name pattern for asset files',
    },
    entryFileNames: {
      anyOf: [{ type: 'string' }],
      description: 'Name pattern for emitted entry chunks',
    },
    chunkFileNames: {
      anyOf: [
        { type: 'string' },
        { $ref: '#/properties/entryFileNames/anyOf/1' },
      ],
      description: 'Name pattern for emitted secondary chunks',
    },
    cssEntryFileNames: {
      anyOf: [
        { type: 'string' },
        { $ref: '#/properties/entryFileNames/anyOf/1' },
      ],
      description: 'Name pattern for emitted css entry chunks',
    },
    cssChunkFileNames: {
      anyOf: [
        { type: 'string' },
        { $ref: '#/properties/entryFileNames/anyOf/1' },
      ],
      description: 'Name pattern for emitted css secondary chunks',
    },
    minify: { type: 'boolean', description: 'Minify the bundled file.' },
    name: { type: 'string', description: 'Name for UMD / IIFE format outputs' },
    globals: {
      type: 'object',
      additionalProperties: { type: 'string' },
      description:
        'Global variable of UMD / IIFE dependencies (syntax: `key=value`)',
    },
    externalLiveBindings: {
      type: 'boolean',
      description: 'external live bindings',
      default: true,
    },
    inlineDynamicImports: {
      type: 'boolean',
      description: 'Inline dynamic imports',
      default: false,
    },
    advancedChunks: {
      type: 'object',
      properties: {
        minSize: { type: 'number', description: 'Minimum size of the chunk' },
        minShareCount: {
          type: 'number',
          description: 'Minimum share count of the chunk',
        },
      },
      additionalProperties: false,
    },
    comments: {
      type: 'string',
      enum: ['none', 'preserve-legal'],
      description: 'Control comments in the output',
    },
  },
  additionalProperties: false,
  $schema: 'http://json-schema.org/draft-07/schema#',
} as unknown as ObjectSchema
