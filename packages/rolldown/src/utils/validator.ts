import * as v from 'valibot'
import { colors } from '../cli/colors'
import { toJsonSchema } from '@valibot/to-json-schema'
import type { PreRenderedChunk } from '../binding'
import type { RolldownPluginOption } from '../plugin'
import type { ObjectSchema } from '../types/schema'
import type { RenderedChunk } from '../types/rolldown-output'
import type { TreeshakingOptions } from '../types/module-side-effects'
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc'

const StringOrRegExpSchema = v.union([v.string(), v.instance(RegExp)])

const LogLevelSchema = v.union([
  v.literal('debug'),
  v.literal('info'),
  v.literal('warn'),
])

const LogLevelOptionSchema = v.union([LogLevelSchema, v.literal('silent')])
const LogLevelWithErrorSchema = v.union([LogLevelSchema, v.literal('error')])

const RollupLogSchema = v.any()
const RollupLogWithStringSchema = v.union([RollupLogSchema, v.string()])

/// --- InputSchema ---

const InputOptionSchema = v.union([
  v.string(),
  v.array(v.string()),
  v.record(v.string(), v.string()),
])

const ExternalSchema = v.union([
  StringOrRegExpSchema,
  v.array(StringOrRegExpSchema),
  v.pipe(
    v.function(),
    v.args(v.tuple([v.string(), v.optional(v.string()), v.boolean()])),
    v.returns(v.nullish(v.boolean())),
  ),
])

const ModuleTypesSchema = v.record(
  v.string(),
  v.union([
    v.literal('base64'),
    v.literal('binary'),
    v.literal('css'),
    v.literal('dataurl'),
    v.literal('empty'),
    v.literal('js'),
    v.literal('json'),
    v.literal('jsx'),
    v.literal('text'),
    v.literal('ts'),
    v.literal('tsx'),
  ]),
)

const JsxOptionsSchema = v.strictObject({
  development: v.pipe(
    v.optional(v.boolean()),
    v.description('Development specific information'),
  ),
  factory: v.pipe(
    v.optional(v.string()),
    v.description('Jsx element transformation'),
  ),
  fragment: v.pipe(
    v.optional(v.string()),
    v.description('Jsx fragment transformation'),
  ),
  importSource: v.pipe(
    v.optional(v.string()),
    v.description(
      'Import the factory of element and fragment if mode is classic',
    ),
  ),
  jsxImportSource: v.pipe(
    v.optional(v.string()),
    v.description(
      'Import the factory of element and fragment if mode is automatic',
    ),
  ),
  mode: v.pipe(
    v.optional(v.union([v.literal('classic'), v.literal('automatic')])),
    v.description('Jsx transformation mode'),
  ),
  refresh: v.pipe(
    v.optional(v.boolean()),
    v.description('React refresh transformation'),
  ),
})

const WatchOptionsSchema = v.strictObject({
  chokidar: v.optional(v.any()),
  exclude: v.optional(
    v.union([StringOrRegExpSchema, v.array(StringOrRegExpSchema)]),
  ),
  include: v.optional(
    v.union([StringOrRegExpSchema, v.array(StringOrRegExpSchema)]),
  ),
  notify: v.pipe(
    v.optional(
      v.strictObject({
        compareContents: v.optional(v.boolean()),
        pollInterval: v.optional(v.number()),
      }),
    ),
    v.description('Notify options'),
  ),
  skipWrite: v.pipe(
    v.optional(v.boolean()),
    v.description('Skip the bundle.write() step'),
  ),
})

const ChecksOptionsSchema = v.strictObject({
  circularDependency: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Wether to emit warnings when detecting circular dependencies',
    ),
  ),
})

const ResolveOptionsSchema = v.strictObject({
  alias: v.optional(
    v.record(v.string(), v.union([v.string(), v.array(v.string())])),
  ),
  aliasFields: v.optional(v.array(v.array(v.string()))),
  conditionNames: v.optional(v.array(v.string())),
  extensionAlias: v.optional(v.record(v.string(), v.array(v.string()))),
  exportsFields: v.optional(v.array(v.array(v.string()))),
  extensions: v.optional(v.array(v.string())),
  mainFields: v.optional(v.array(v.string())),
  mainFiles: v.optional(v.array(v.string())),
  modules: v.optional(v.array(v.string())),
  symlinks: v.optional(v.boolean()),
  tsconfigFilename: v.optional(v.string()),
})

const TreeshakingOptionsSchema = v.union([
  v.boolean(),
  v.looseObject({ annotations: v.optional(v.boolean()) }),
])

const OnLogSchema = v.pipe(
  v.function(),
  v.args(
    v.tuple([
      LogLevelSchema,
      RollupLogSchema,
      v.pipe(
        v.function(),
        v.args(v.tuple([LogLevelWithErrorSchema, RollupLogWithStringSchema])),
      ),
    ]),
  ),
)

const OnwarnSchema = v.pipe(
  v.function(),
  v.args(
    v.tuple([
      RollupLogSchema,
      v.pipe(
        v.function(),
        v.args(
          v.tuple([
            v.union([
              RollupLogWithStringSchema,
              v.pipe(v.function(), v.returns(RollupLogWithStringSchema)),
            ]),
          ]),
        ),
      ),
    ]),
  ),
)

const InputOptionsSchema = v.strictObject({
  input: v.optional(InputOptionSchema),
  plugins: v.optional(v.custom<RolldownPluginOption>(() => true)),
  external: v.optional(ExternalSchema),
  resolve: v.optional(ResolveOptionsSchema),
  cwd: v.pipe(
    v.optional(v.string()),
    v.description('Current working directory'),
  ),
  platform: v.pipe(
    v.optional(
      v.union([v.literal('browser'), v.literal('neutral'), v.literal('node')]),
    ),
    v.description(
      `Platform for which the code should be generated (node, ${colors.underline('browser')}, neutral)`,
    ),
  ),
  shimMissingExports: v.pipe(
    v.optional(v.boolean()),
    v.description('Create shim variables for missing exports'),
  ),
  treeshake: v.optional(TreeshakingOptionsSchema),
  logLevel: v.pipe(
    v.optional(LogLevelOptionSchema),
    v.description(
      `Log level (${colors.dim('silent')}, ${colors.underline(colors.gray('info'))}, debug, ${colors.yellow('warn')})`,
    ),
  ),
  onLog: v.optional(OnLogSchema),
  onwarn: v.optional(OnwarnSchema),
  moduleTypes: v.pipe(
    v.optional(ModuleTypesSchema),
    v.description('Module types for customized extensions'),
  ),
  experimental: v.optional(
    v.strictObject({
      disableLiveBindings: v.optional(v.boolean()),
      enableComposingJsPlugins: v.optional(v.boolean()),
      resolveNewUrlToAsset: v.optional(v.boolean()),
      strictExecutionOrder: v.optional(v.boolean()),
    }),
  ),
  define: v.pipe(
    v.optional(v.record(v.string(), v.string())),
    v.description('Define global variables'),
  ),
  inject: v.optional(
    v.record(
      v.string(),
      v.union([v.string(), v.tuple([v.string(), v.string()])]),
    ),
  ),
  profilerNames: v.optional(v.boolean()),
  jsx: v.optional(JsxOptionsSchema),
  watch: v.optional(v.union([WatchOptionsSchema, v.literal(false)])),
  dropLabels: v.pipe(
    v.optional(v.array(v.string())),
    v.description('Remove labeled statements with these label names'),
  ),
  checks: v.optional(ChecksOptionsSchema),
})

const InputCliOverrideSchema = v.strictObject({
  external: v.pipe(
    v.optional(v.array(v.string())),
    v.description(
      'Comma-separated list of module ids to exclude from the bundle `<module-id>,...`',
    ),
  ),
  inject: v.pipe(
    v.optional(v.record(v.string(), v.string())),
    v.description('Inject import statements on demand'),
  ),
  treeshake: v.pipe(
    v.optional(v.boolean(), true),
    v.description('enable treeshaking'),
  ),
})

const InputCliOptionsSchema = v.omit(
  v.strictObject({
    ...InputOptionsSchema.entries,
    ...InputCliOverrideSchema.entries,
  }),
  [
    'input',
    'plugins',
    'onwarn',
    'onLog',
    'resolve',
    'experimental',
    'profilerNames',
    'watch',
  ],
)

/// --- OutputSchema ---

enum ESTarget {
  ES6 = 'es6',
  ES2015 = 'es2015',
  ES2016 = 'es2016',
  ES2017 = 'es2017',
  ES2018 = 'es2018',
  ES2019 = 'es2019',
  ES2020 = 'es2020',
  ES2021 = 'es2021',
  ES2022 = 'es2022',
  ES2023 = 'es2023',
  ES2024 = 'es2024',
  ESNext = 'esnext',
}

const ModuleFormatSchema = v.union([
  v.literal('es'),
  v.literal('cjs'),
  v.literal('esm'),
  v.literal('module'),
  v.literal('commonjs'),
  v.literal('iife'),
  v.literal('umd'),
])

const AddonFunctionSchema = v.pipe(
  v.function(),
  v.args(v.tuple([v.custom<RenderedChunk>(() => true)])),
  v.returnsAsync(
    v.unionAsync([
      v.string(),
      v.pipeAsync(v.promise(), v.awaitAsync(), v.string()),
    ]),
  ),
)

const ChunkFileNamesSchema = v.union([
  v.string(),
  v.pipe(
    v.function(),
    v.args(v.tuple([v.custom<PreRenderedChunk>(() => true)])),
    v.returns(v.string()),
  ),
])

const GlobalsFunctionSchema = v.pipe(
  v.function(),
  v.args(v.tuple([v.string()])),
  v.returns(v.string()),
)

const AdvancedChunksSchema = v.strictObject({
  minSize: v.optional(v.number()),
  minShareCount: v.optional(v.number()),
  groups: v.optional(
    v.array(
      v.strictObject({
        name: v.string(),
        test: v.optional(v.union([v.string(), v.instance(RegExp)])),
        priority: v.optional(v.number()),
        minSize: v.optional(v.number()),
        minShareCount: v.optional(v.number()),
      }),
    ),
  ),
})

const OutputOptionsSchema = v.strictObject({
  dir: v.pipe(
    v.optional(v.string()),
    v.description('Output directory, defaults to `dist` if `file` is not set'),
  ),
  file: v.pipe(v.optional(v.string()), v.description('Single output file')),
  exports: v.pipe(
    v.optional(
      v.union([
        v.literal('auto'),
        v.literal('named'),
        v.literal('default'),
        v.literal('none'),
      ]),
    ),
    v.description(
      `Specify a export mode (${colors.underline('auto')}, named, default, none)`,
    ),
  ),
  hashCharacters: v.pipe(
    v.optional(
      v.union([v.literal('base64'), v.literal('base36'), v.literal('hex')]),
    ),
    v.description('Use the specified character set for file hashes'),
  ),
  format: v.pipe(
    v.optional(ModuleFormatSchema),
    v.description(
      `Output format of the generated bundle (supports ${colors.underline('esm')}, cjs, and iife)`,
    ),
  ),

  sourcemap: v.pipe(
    v.optional(
      v.union([v.boolean(), v.literal('inline'), v.literal('hidden')]),
    ),
    v.description(
      `Generate sourcemap (\`-s inline\` for inline, or ${colors.bold('pass the `-s` on the last argument if you want to generate `.map` file')})`,
    ),
  ),
  sourcemapIgnoreList: v.optional(
    v.union([v.boolean(), v.custom<SourcemapIgnoreListOption>(() => true)]),
  ),
  sourcemapPathTransform: v.optional(
    v.custom<SourcemapPathTransformOption>(() => true),
  ),
  banner: v.optional(v.union([v.string(), AddonFunctionSchema])),
  footer: v.optional(v.union([v.string(), AddonFunctionSchema])),
  intro: v.optional(v.union([v.string(), AddonFunctionSchema])),
  outro: v.optional(v.union([v.string(), AddonFunctionSchema])),
  extend: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Extend global variable defined by name in IIFE / UMD formats',
    ),
  ),
  esModule: v.optional(v.union([v.boolean(), v.literal('if-default-prop')])),
  assetFileNames: v.pipe(
    v.optional(v.string()),
    v.description('Name pattern for asset files'),
  ),
  entryFileNames: v.optional(ChunkFileNamesSchema),
  chunkFileNames: v.optional(ChunkFileNamesSchema),
  cssEntryFileNames: v.optional(ChunkFileNamesSchema),
  cssChunkFileNames: v.optional(ChunkFileNamesSchema),
  minify: v.pipe(
    v.optional(v.boolean()),
    v.description('Minify the bundled file'),
  ),
  name: v.pipe(
    v.optional(v.string()),
    v.description('Name for UMD / IIFE format outputs'),
  ),
  globals: v.pipe(
    v.optional(
      v.union([v.record(v.string(), v.string()), GlobalsFunctionSchema]),
    ),
    v.description(
      'Global variable of UMD / IIFE dependencies (syntax: `key=value`)',
    ),
  ),
  externalLiveBindings: v.pipe(
    v.optional(v.boolean(), true),
    v.description('external live bindings'),
  ),
  inlineDynamicImports: v.pipe(
    v.optional(v.boolean(), false),
    v.description('Inline dynamic imports'),
  ),
  advancedChunks: v.optional(AdvancedChunksSchema),
  comments: v.pipe(
    v.optional(v.union([v.literal('none'), v.literal('preserve-legal')])),
    v.description('Control comments in the output'),
  ),
  target: v.pipe(
    v.optional(v.enum(ESTarget)),
    v.description('The JavaScript target environment'),
  ),
})

const getAddonDescription = (
  placement: 'bottom' | 'top',
  wrapper: 'inside' | 'outside',
) => {
  return `Code to insert the ${colors.bold(placement)} of the bundled file (${colors.bold(wrapper)} the wrapper function)`
}

const OutputCliOverrideSchema = v.strictObject({
  // Reject all functions in CLI
  entryFileNames: v.pipe(
    v.optional(v.string()),
    v.description('Name pattern for emitted entry chunks'),
  ),
  chunkFileNames: v.pipe(
    v.optional(v.string()),
    v.description('Name pattern for emitted secondary chunks'),
  ),
  cssEntryFileNames: v.pipe(
    v.optional(v.string()),
    v.description('Name pattern for emitted css entry chunks'),
  ),
  cssChunkFileNames: v.pipe(
    v.optional(v.string()),
    v.description('Name pattern for emitted css secondary chunks'),
  ),
  banner: v.pipe(
    v.optional(v.string()),
    v.description(getAddonDescription('top', 'outside')),
  ),
  footer: v.pipe(
    v.optional(v.string()),
    v.description(getAddonDescription('bottom', 'outside')),
  ),
  intro: v.pipe(
    v.optional(v.string()),
    v.description(getAddonDescription('top', 'inside')),
  ),
  outro: v.pipe(
    v.optional(v.string()),
    v.description(getAddonDescription('bottom', 'inside')),
  ),
  // It is hard to handle the union type in json schema, so use this first.
  esModule: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Always generate `__esModule` marks in non-ESM formats, defaults to `if-default-prop` (use `--no-esModule` to always disable)',
    ),
  ),
  globals: v.pipe(
    v.optional(v.record(v.string(), v.string())),
    v.description(
      'Global variable of UMD / IIFE dependencies (syntax: `key=value`)',
    ),
  ),
  advancedChunks: v.pipe(
    v.optional(
      v.strictObject({
        minSize: v.pipe(
          v.optional(v.number()),
          v.description('Minimum size of the chunk'),
        ),
        minShareCount: v.pipe(
          v.optional(v.number()),
          v.description('Minimum share count of the chunk'),
        ),
      }),
    ),
    v.description(
      'Global variable of UMD / IIFE dependencies (syntax: `key=value`)',
    ),
  ),
})

const OutputCliOptionsSchema = v.omit(
  v.strictObject({
    ...OutputOptionsSchema.entries,
    ...OutputCliOverrideSchema.entries,
  }),
  ['sourcemapIgnoreList', 'sourcemapPathTransform'],
)

/// --- CliSchema ---

const CliOptionsSchema = v.strictObject({
  config: v.pipe(
    v.optional(v.union([v.string(), v.boolean()])),
    v.description('Path to the config file (default: `rolldown.config.js`)'),
  ),
  help: v.pipe(v.optional(v.boolean()), v.description('Show help')),
  version: v.pipe(
    v.optional(v.boolean()),

    v.description('Show version number'),
  ),
  watch: v.pipe(
    v.optional(v.boolean()),
    v.description('Watch files in bundle and rebuild on changes'),
  ),
  ...InputCliOptionsSchema.entries,
  ...OutputCliOptionsSchema.entries,
})

export function validateTreeShakingOptions(options: TreeshakingOptions): void {
  v.parse(TreeshakingOptionsSchema, options)
}

export function validateCliOptions<T>(options: T): [T, string[]?] {
  let parsed = v.safeParse(CliOptionsSchema, options)

  return [
    parsed.output as T,
    parsed.issues
      ?.map((issue) => issue.path?.join(', '))
      .filter((v) => v !== undefined),
  ]
}

export function getInputCliKeys(): string[] {
  return v.keyof(InputCliOptionsSchema).options
}

export function getOutputCliKeys(): string[] {
  return v.keyof(OutputCliOptionsSchema).options
}

export function getJsonSchema(): ObjectSchema {
  return toJsonSchema(CliOptionsSchema) as ObjectSchema
}
