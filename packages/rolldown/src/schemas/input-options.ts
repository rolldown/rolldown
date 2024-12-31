import * as v from 'valibot'
import * as valibotExt from './valibot-ext'
import { colors } from '../cli/colors'
import type { RolldownPluginOption } from '../plugin'

const LogLevelSchema = v.union([
  v.literal('debug'),
  v.literal('info'),
  v.literal('warn'),
])

const LogLevelOptionSchema = v.union([LogLevelSchema, v.literal('silent')])
const LogLevelWithErrorSchema = v.union([LogLevelSchema, v.literal('error')])

const RollupLogSchema = v.any()
const RollupLogWithStringSchema = v.union([RollupLogSchema, v.string()])

const inputOptionSchema = v.union([
  v.string(),
  v.array(v.string()),
  v.record(v.string(), v.string()),
])

const externalSchema = v.union([
  valibotExt.stringOrRegExp(),
  v.array(valibotExt.stringOrRegExp()),
  v.pipe(
    v.function(),
    v.args(v.tuple([v.string(), v.optional(v.string()), v.boolean()])),
    v.returns(valibotExt.voidNullableWith(v.boolean())),
  ),
])

const moduleTypesSchema = v.record(
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

const jsxOptionsSchema = v.strictObject({
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

const stringOrRegExpSchema = v.union([
  valibotExt.stringOrRegExp(),
  v.array(valibotExt.stringOrRegExp()),
])

const watchOptionsSchema = v.strictObject({
  chokidar: v.optional(v.any()),
  exclude: v.optional(stringOrRegExpSchema),
  include: v.optional(stringOrRegExpSchema),
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

const checksOptionsSchema = v.strictObject({
  circularDependency: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Wether to emit warnings when detecting circular dependencies',
    ),
  ),
})

const resolveOptionsSchema = v.strictObject({
  alias: v.optional(
    v.record(v.string(), v.union([v.string(), v.array(v.string())])),
  ),
  aliasFields: v.optional(v.array(v.array(v.string()))),
  conditionNames: valibotExt.optionalStringArray(),
  extensionAlias: v.optional(v.record(v.string(), v.array(v.string()))),
  exportsFields: v.optional(v.array(v.array(v.string()))),
  extensions: valibotExt.optionalStringArray(),
  mainFields: valibotExt.optionalStringArray(),
  mainFiles: valibotExt.optionalStringArray(),
  modules: valibotExt.optionalStringArray(),
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

const inputOptionsSchema = v.strictObject({
  input: v.optional(inputOptionSchema),
  plugins: v.optional(valibotExt.phantom<RolldownPluginOption>()),
  external: v.optional(externalSchema),
  resolve: v.optional(resolveOptionsSchema),
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
    v.optional(moduleTypesSchema),
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
  jsx: v.optional(jsxOptionsSchema),
  watch: v.optional(v.union([watchOptionsSchema, v.literal(false)])),
  dropLabels: v.pipe(
    v.optional(v.array(v.string())),
    v.description('Remove labeled statements with these label names'),
  ),

  checks: v.optional(checksOptionsSchema),
})

const cliOverrideSchema = v.strictObject({
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

const inputCliOptionsSchema = v.omit(
  v.strictObject({
    ...inputOptionsSchema.entries,
    ...cliOverrideSchema.entries,
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

export function validateInputCliOptions(options: any): boolean {
  return v.safeParse(inputCliOptionsSchema, options).success
}
