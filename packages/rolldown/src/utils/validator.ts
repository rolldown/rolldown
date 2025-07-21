import { toJsonSchema } from '@valibot/to-json-schema';
import colors from 'ansis';
import * as v from 'valibot';
import type { PreRenderedChunk } from '../binding';
import type { PreRenderedAsset } from '../options/output-options';
import type {
  RolldownOutputPluginOption,
  RolldownPluginOption,
} from '../plugin';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
import type { RenderedChunk } from '../types/rolldown-output';
import type { ObjectSchema } from '../types/schema';

const StringOrRegExpSchema = v.union([v.string(), v.instance(RegExp)]);

const LogLevelSchema = v.union([
  v.literal('debug'),
  v.literal('info'),
  v.literal('warn'),
]);

const LogLevelOptionSchema = v.union([LogLevelSchema, v.literal('silent')]);
const LogLevelWithErrorSchema = v.union([LogLevelSchema, v.literal('error')]);

const RollupLogSchema = v.any();
const RollupLogWithStringSchema = v.union([RollupLogSchema, v.string()]);

/// --- InputSchema ---

const InputOptionSchema = v.union([
  v.string(),
  v.array(v.string()),
  v.record(v.string(), v.string()),
]);

const ExternalSchema = v.union([
  StringOrRegExpSchema,
  v.array(StringOrRegExpSchema),
  v.pipe(
    v.function(),
    v.args(v.tuple([v.string(), v.optional(v.string()), v.boolean()])),
    v.returns(v.nullish(v.boolean())),
  ),
]);

const ModuleTypesSchema = v.record(
  v.string(),
  v.union([
    v.literal('asset'),
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
);

const JsxOptionsSchema = v.strictObject({
  runtime: v.pipe(
    v.optional(v.union([
      v.literal('classic'),
      v.literal('automatic'),
    ])),
    v.description('Which runtime to use'),
  ),
  development: v.pipe(
    v.optional(v.boolean()),
    v.description('Development specific information'),
  ),
  throwIfNamespace: v.pipe(
    v.optional(v.string()),
    v.description(
      'Toggles whether to throw an error when a tag name uses an XML namespace',
    ),
  ),
  importSource: v.pipe(
    v.optional(v.string()),
    v.description(
      'Import the factory of element and fragment if mode is classic',
    ),
  ),
  pragma: v.pipe(
    v.optional(v.string()),
    v.description('Jsx element transformation'),
  ),
  pragmaFlag: v.pipe(
    v.optional(
      v.string(),
    ),
    v.description('Jsx fragment transformation'),
  ),
  refresh: v.pipe(
    v.optional(v.boolean()),
    v.description('Enable react fast refresh'),
  ),
});

const RollupJsxOptionsSchema = v.strictObject({
  mode: v.optional(v.union([
    v.literal('classic'),
    v.literal('automatic'),
    v.literal('preserve'),
  ])),
  factory: v.optional(v.string()),
  fragment: v.optional(v.string()),
  importSource: v.optional(v.string()),
  jsxImportSource: v.optional(v.string()),
});

const HelperModeSchema = v.union([v.literal('Runtime'), v.literal('External')]);

const DecoratorOptionSchema = v.object({
  legacy: v.optional(v.boolean()),
  emitDecoratorMetadata: v.optional(v.boolean()),
});

const HelpersSchema = v.object({
  mode: v.optional(HelperModeSchema),
});

const RewriteImportExtensionsSchema = v.union([
  v.literal('rewrite'),
  v.literal('remove'),
  v.boolean(),
]);
const TypescriptSchema = v.object({
  jsxPragma: v.optional(v.string()),
  jsxPragmaFrag: v.optional(v.string()),
  onlyRemoveTypeImports: v.optional(v.boolean()),
  allowNamespaces: v.optional(v.boolean()),
  allowDeclareFields: v.optional(v.boolean()),
  declaration: v.optional(
    v.object({
      stripInternal: v.optional(v.boolean()),
      sourcemap: v.optional(v.boolean()),
    }),
  ),
  rewriteImportExtensions: v.optional(RewriteImportExtensionsSchema),
});
const AssumptionsSchema = v.object({
  ignoreFunctionLength: v.optional(v.boolean()),
  noDocumentAll: v.optional(v.boolean()),
  objectRestNoSymbols: v.optional(v.boolean()),
  pureGetters: v.optional(v.boolean()),
  setPublicClassFields: v.optional(v.boolean()),
});
const TransformOptionsSchema = v.object({
  assumptions: v.optional(AssumptionsSchema),
  typescript: v.optional(TypescriptSchema),
  helpers: v.optional(HelpersSchema),
  decorators: v.optional(DecoratorOptionSchema),
  jsx: v.optional(JsxOptionsSchema),
  target: v.pipe(
    v.optional(v.union([v.string(), v.array(v.string())])),
    v.description('The JavaScript target environment'),
  ),
});

const WatchOptionsSchema = v.strictObject({
  chokidar: v.optional(
    v.never(
      `The "watch.chokidar" option is deprecated, please use "watch.notify" instead of it`,
    ),
  ),
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
  buildDelay: v.pipe(
    v.optional(v.number()),
    v.description('Throttle watch rebuilds'),
  ),
});

const ChecksOptionsSchema = v.strictObject({
  circularDependency: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting circular dependency',
    ),
  ),
  eval: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting eval',
    ),
  ),
  missingGlobalName: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting missing global name',
    ),
  ),
  missingNameOptionForIifeExport: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting missing name option for iife export',
    ),
  ),
  mixedExport: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting mixed export',
    ),
  ),
  unresolvedEntry: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting unresolved entry',
    ),
  ),
  unresolvedImport: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting unresolved import',
    ),
  ),
  filenameConflict: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting filename conflict',
    ),
  ),
  commonJsVariableInEsm: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting common js variable in esm',
    ),
  ),
  importIsUndefined: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting import is undefined',
    ),
  ),
  emptyImportMeta: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting empty import meta',
    ),
  ),
  configurationFieldConflict: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting configuration field conflict',
    ),
  ),
});

const MinifyOptionsSchema = v.strictObject({
  mangle: v.optional(v.boolean()),
  compress: v.optional(v.boolean()),
  removeWhitespace: v.optional(v.boolean()),
});

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
  yarnPnp: v.optional(v.boolean()),
});

// TODO: moduleSideEffects
const TreeshakingOptionsSchema = v.union([
  v.boolean(),
  v.looseObject({
    annotations: v.optional(v.boolean()),
    manualPureFunctions: v.optional(v.array(v.string())),
    unknownGlobalSideEffects: v.optional(v.boolean()),
    commonjs: v.optional(v.boolean()),
  }),
]);

const OptimizationOptionsSchema = v.strictObject({
  inlineConst: v.pipe(
    v.optional(v.boolean()),
    v.description('Enable crossmodule constant inlining'),
  ),
});

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
);

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
);

const HmrSchema = v.union([
  v.boolean(),
  v.strictObject({
    port: v.optional(v.number()),
    host: v.optional(v.string()),
    implement: v.optional(v.string()),
  }),
]);

const InputOptionsSchema = v.strictObject({
  input: v.optional(InputOptionSchema),
  plugins: v.optional(v.custom<RolldownPluginOption>(() => true)),
  external: v.optional(ExternalSchema),
  makeAbsoluteExternalsRelative: v.optional(
    v.union([v.boolean(), v.literal('ifRelativeSource')]),
  ),
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
      `Platform for which the code should be generated (node, ${
        colors.underline('browser')
      }, neutral)`,
    ),
  ),
  shimMissingExports: v.pipe(
    v.optional(v.boolean()),
    v.description('Create shim variables for missing exports'),
  ),
  treeshake: v.optional(TreeshakingOptionsSchema),
  optimization: v.optional(OptimizationOptionsSchema),
  logLevel: v.pipe(
    v.optional(LogLevelOptionSchema),
    v.description(
      `Log level (${colors.dim('silent')}, ${
        colors.underline(colors.gray('info'))
      }, debug, ${colors.yellow('warn')})`,
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
      viteMode: v.optional(v.boolean()),
      resolveNewUrlToAsset: v.optional(v.boolean()),
      strictExecutionOrder: v.optional(v.boolean()),
      onDemandWrapping: v.optional(v.boolean()),
      incrementalBuild: v.optional(v.boolean()),
      hmr: v.optional(HmrSchema),
      attachDebugInfo: v.optional(v.union([
        v.literal('none'),
        v.literal('simple'),
        v.literal('full'),
      ])),
      chunkModulesOrder: v.optional(v.union([
        v.literal('module-id'),
        v.literal('exec-order'),
      ])),
      chunkImportMap: v.optional(v.boolean()),
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
  jsx: v.optional(
    v.union([
      v.literal(false),
      v.literal('react'),
      v.literal('react-jsx'),
      v.literal('preserve'),
      RollupJsxOptionsSchema,
    ]),
  ),
  transform: v.optional(TransformOptionsSchema),
  watch: v.optional(v.union([WatchOptionsSchema, v.literal(false)])),
  dropLabels: v.pipe(
    v.optional(v.array(v.string())),
    v.description('Remove labeled statements with these label names'),
  ),
  checks: v.optional(ChecksOptionsSchema),
  keepNames: v.pipe(
    v.optional(v.boolean()),
    v.description('Keep function/class name'),
  ),
  debug: v.pipe(
    v.optional(v.object({
      sessionId: v.pipe(
        v.optional(v.string()),
        v.description('Used to name the build.'),
      ),
    })),
    v.description(
      'Enable debug mode. Emit debug information to disk. This might slow down the build process significantly.',
    ),
  ),
  preserveEntrySignatures: v.pipe(
    v.optional(v.union([
      v.literal('strict'),
      v.literal('allow-extension'),
      v.literal('exports-only'),
      v.literal(false),
    ])),
  ),
});

const InputCliOverrideSchema = v.strictObject({
  input: v.pipe(
    v.optional(v.array(v.string())),
    v.description('Entry file'),
  ),
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
    v.optional(v.boolean()),
    v.description('enable treeshaking'),
  ),
  makeAbsoluteExternalsRelative: v.pipe(
    v.optional(v.boolean()),
    v.description('Prevent normalization of external imports'),
  ),
  jsx: v.pipe(
    v.optional(
      v.union([
        v.literal(false),
        v.literal('react'),
        v.literal('react-jsx'),
        v.literal('preserve'),
      ]),
    ),
    v.description('Jsx options preset'),
  ),
  preserveEntrySignatures: v.pipe(
    v.optional(v.union([
      v.literal(false),
    ])),
    v.description('Avoid facade chunks for entry points'),
  ),
});

const InputCliOptionsSchema = v.omit(
  v.strictObject({
    ...InputOptionsSchema.entries,
    ...InputCliOverrideSchema.entries,
  }),
  [
    'plugins',
    'onwarn',
    'onLog',
    'resolve',
    'experimental',
    'profilerNames',
    'watch',
  ],
);

/// --- OutputSchema ---

const ModuleFormatSchema = v.union([
  v.literal('es'),
  v.literal('cjs'),
  v.literal('esm'),
  v.literal('module'),
  v.literal('commonjs'),
  v.literal('iife'),
  v.literal('umd'),
]);

const AddonFunctionSchema = v.pipe(
  v.function(),
  v.args(v.tuple([v.custom<RenderedChunk>(() => true)])),
  v.returnsAsync(
    v.unionAsync([
      v.string(),
      v.pipeAsync(v.promise(), v.awaitAsync(), v.string()),
    ]),
  ),
);

const ChunkFileNamesSchema = v.union([
  v.string(),
  v.pipe(
    v.function(),
    v.args(v.tuple([v.custom<PreRenderedChunk>(() => true)])),
    v.returns(v.string()),
  ),
]);

const AssetFileNamesSchema = v.union([
  v.string(),
  v.pipe(
    v.function(),
    v.args(v.tuple([v.custom<PreRenderedAsset>(() => true)])),
    v.returns(v.string()),
  ),
]);

const SanitizeFileNameSchema = v.union([
  v.boolean(),
  v.pipe(v.function(), v.args(v.tuple([v.string()])), v.returns(v.string())),
]);

const GlobalsFunctionSchema = v.pipe(
  v.function(),
  v.args(v.tuple([v.string()])),
  v.returns(v.string()),
);

const AdvancedChunksSchema = v.strictObject({
  minSize: v.optional(v.number()),
  maxSize: v.optional(v.number()),
  minModuleSize: v.optional(v.number()),
  maxModuleSize: v.optional(v.number()),
  minShareCount: v.optional(v.number()),
  groups: v.optional(
    v.array(
      v.strictObject({
        name: v.union([
          v.string(),
          v.pipe(
            v.function(),
            v.args(v.tuple([v.string()])),
            v.returns(v.nullish(v.string())),
          ),
        ]),
        test: v.optional(
          v.union([
            v.string(),
            v.instance(RegExp),
            v.pipe(
              v.function(),
              v.args(v.tuple([v.string()])),
              v.returns(v.union([v.nullish(v.boolean()), v.void()])),
            ),
          ]),
        ),
        priority: v.optional(v.number()),
        minSize: v.optional(v.number()),
        minShareCount: v.optional(v.number()),
        maxSize: v.optional(v.number()),
        minModuleSize: v.optional(v.number()),
        maxModuleSize: v.optional(v.number()),
      }),
    ),
  ),
});

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
      `Specify a export mode (${
        colors.underline('auto')
      }, named, default, none)`,
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
      `Output format of the generated bundle (supports ${
        colors.underline('esm')
      }, cjs, and iife)`,
    ),
  ),
  sourcemap: v.pipe(
    v.optional(
      v.union([v.boolean(), v.literal('inline'), v.literal('hidden')]),
    ),
    v.description(
      `Generate sourcemap (\`-s inline\` for inline, or ${
        colors.bold(
          'pass the `-s` on the last argument if you want to generate `.map` file',
        )
      })`,
    ),
  ),
  sourcemapDebugIds: v.pipe(
    v.optional(v.boolean()),
    v.description('Inject sourcemap debug IDs'),
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
  assetFileNames: v.optional(AssetFileNamesSchema),
  entryFileNames: v.optional(ChunkFileNamesSchema),
  chunkFileNames: v.optional(ChunkFileNamesSchema),
  cssEntryFileNames: v.optional(ChunkFileNamesSchema),
  cssChunkFileNames: v.optional(ChunkFileNamesSchema),
  sanitizeFileName: v.optional(SanitizeFileNameSchema),
  minify: v.pipe(
    v.optional(
      v.union([v.boolean(), v.string('dce-only'), MinifyOptionsSchema]),
    ),
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
    v.optional(v.boolean()),
    v.description('external live bindings'),
  ),
  inlineDynamicImports: v.pipe(
    v.optional(v.boolean()),
    v.description('Inline dynamic imports'),
  ),
  manualChunks: v.optional(
    v.pipe(
      v.function(),
      v.args(v.tuple([v.string(), v.object({})])),
      v.returns(v.union([v.string(), v.nullish(v.string())])),
    ),
  ),
  advancedChunks: v.optional(AdvancedChunksSchema),
  legalComments: v.pipe(
    v.optional(v.union([v.literal('none'), v.literal('inline')])),
    v.description('Control comments in the output'),
  ),
  plugins: v.optional(v.custom<RolldownOutputPluginOption>(() => true)),
  polyfillRequire: v.pipe(
    v.optional(v.boolean()),
    v.description('Disable require polyfill injection'),
  ),
  hoistTransitiveImports: v.optional(
    v.custom<boolean, () => string>((input) => {
      if (input) {
        return false;
      }
      return true;
    }, () => `The 'true' value is not supported`),
  ),
  preserveModules: v.pipe(
    v.optional(v.boolean()),
    v.description('Preserve module structure'),
  ),
  preserveModulesRoot: v.pipe(
    v.optional(v.string()),
    v.description('Put preserved modules under this path at root level'),
  ),
  virtualDirname: v.optional(v.string()),
  minifyInternalExports: v.pipe(
    v.optional(v.boolean()),
    v.description('Minify internal exports'),
  ),
});

const getAddonDescription = (
  placement: 'bottom' | 'top',
  wrapper: 'inside' | 'outside',
) => {
  return `Code to insert the ${colors.bold(placement)} of the bundled file (${
    colors.bold(wrapper)
  } the wrapper function)`;
};

const OutputCliOverrideSchema = v.strictObject({
  // Reject all functions in CLI
  assetFileNames: v.pipe(
    v.optional(v.string()),
    v.description('Name pattern for asset files'),
  ),
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
  sanitizeFileName: v.pipe(
    v.optional(v.boolean()),
    v.description('Sanitize file name'),
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
  minify: v.pipe(
    v.optional(v.boolean()),
    v.description('Minify the bundled file'),
  ),
});

const OutputCliOptionsSchema = v.omit(
  v.strictObject({
    ...OutputOptionsSchema.entries,
    ...OutputCliOverrideSchema.entries,
  }),
  [
    'sourcemapIgnoreList',
    'sourcemapPathTransform',
    'plugins',
    'hoistTransitiveImports',
  ],
);

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
});

export function validateCliOptions<T>(options: T): [T, string[]?] {
  let parsed = v.safeParse(CliOptionsSchema, options);

  return [
    parsed.output as T,
    parsed.issues?.map((issue) => {
      const option = issue.path?.map((pathItem) => pathItem.key).join(' ');
      return `Invalid value for option ${option}: ${issue.message}`;
    }),
  ];
}

type HelperMsgRecord = Record<string, { ignored?: boolean; msg?: string }>;

const inputHelperMsgRecord: HelperMsgRecord = {
  output: { ignored: true }, // Ignore the output key
};
const outputHelperMsgRecord: HelperMsgRecord = {};

export function validateOption<T>(key: 'input' | 'output', options: T): void {
  if (globalThis.process?.env?.ROLLUP_TEST) return;
  let parsed = v.safeParse(
    key === 'input' ? InputOptionsSchema : OutputOptionsSchema,
    options,
  );

  if (!parsed.success) {
    const errors = parsed.issues
      .map((issue) => {
        const issuePaths = issue.path!.map((path) => path.key);
        let issueMsg = issue.message;
        // For issue in union type, ref https://valibot.dev/guides/unions/
        // - the received is not matched with the all the sub typing
        // - one sub typing is matched, but it is has issue, we need to find the matched sub issue
        if (issue.type === 'union') {
          const subIssue = issue.issues?.find(
            (i) => !(i.type !== issue.received && i.input === issue.input),
          );
          if (subIssue) {
            if (subIssue.path) {
              issuePaths.push(subIssue.path.map((path) => path.key));
            }
            issueMsg = subIssue.message;
          }
        }
        const stringPath = issuePaths.join('.');
        const helper = key === 'input'
          ? inputHelperMsgRecord[stringPath]
          : outputHelperMsgRecord[stringPath];
        if (helper && helper.ignored) {
          return '';
        }
        return `- For the "${stringPath}". ${issueMsg}. ${
          helper ? helper.msg : ''
        }`;
      })
      .filter(Boolean);
    if (errors.length) {
      console.warn(`Warning validate ${key} options.\n` + errors.join('\n'));
    }
  }
}

export function getInputCliKeys(): string[] {
  return v.keyof(InputCliOptionsSchema).options;
}

export function getOutputCliKeys(): string[] {
  return v.keyof(OutputCliOptionsSchema).options;
}

export function getJsonSchema(): ObjectSchema {
  return toJsonSchema(CliOptionsSchema, {
    // errorMode: 'ignore' is set to ignore `never` schema
    // there's no way to suppress the error one-by-one
    // https://github.com/fabian-hiller/valibot/issues/1062
    errorMode: 'ignore',
  }) as ObjectSchema;
}
