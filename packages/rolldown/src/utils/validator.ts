import * as v from 'valibot';
import type {
  CompressOptions,
  MangleOptions,
  MangleOptionsKeepNames,
  MinifyOptions,
  PreRenderedChunk,
} from '../binding';
import type { LogOrStringHandler } from '../log/logging';
import type {
  ExternalOption,
  ExternalOptionFunction,
  InputOptions,
  OnLogFunction,
  OnwarnFunction,
  OptimizationOptions,
} from '../options/input-options';
import type {
  AddonFunction,
  AdvancedChunksNameFunction,
  AdvancedChunksTestFunction,
  AssetFileNamesFunction,
  ChunkFileNamesFunction,
  GlobalsFunction,
  ManualChunksFunction,
  OutputOptions,
  PathsFunction,
  PreRenderedAsset,
  SanitizeFileNameFunction,
} from '../options/output-options';
import type {
  RolldownOutputPluginOption,
  RolldownPluginOption,
} from '../plugin';
import type {
  SourcemapIgnoreListOption,
  SourcemapPathTransformOption,
} from '../types/misc';
import type { RenderedChunk } from '../types/rolldown-output';
import type { AnyFn } from '../types/utils';
import { flattenValibotSchema } from './flatten-valibot-schema';
import { styleText } from './style-text';

const StringOrRegExpSchema = v.union([v.string(), v.instance(RegExp)]);

// A helper function to create a valibot schema for a function. It assumes the
// passed function is a properly defined function type with expected argument and return
// type.
// See https://github.com/fabian-hiller/valibot/issues/1342
function vFunction<T extends AnyFn>(): v.GenericSchema<T> {
  return v.function() as unknown as v.GenericSchema<T>;
}

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

const ExternalOptionFunctionSchema = v.pipe(
  vFunction<ExternalOptionFunction>(),
  v.args(v.tuple([v.string(), v.optional(v.string()), v.boolean()])),
  v.returns(v.nullish(v.boolean())),
) satisfies v.GenericSchema<ExternalOptionFunction>;

const ExternalOptionSchema = v.union([
  StringOrRegExpSchema,
  v.array(StringOrRegExpSchema),
  ExternalOptionFunctionSchema,
]) satisfies v.GenericSchema<ExternalOption>;

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
    v.optional(v.boolean()),
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
  pragmaFrag: v.pipe(
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
  jsx: v.optional(
    v.union([
      v.literal(false),
      v.literal('preserve'),
      v.literal('react'),
      v.literal('react-jsx'),
      JsxOptionsSchema,
    ]),
  ),
  target: v.pipe(
    v.optional(v.union([v.string(), v.array(v.string())])),
    v.description('The JavaScript target environment'),
  ),
  define: v.optional(v.record(v.string(), v.string())),
  inject: v.optional(
    v.record(
      v.string(),
      v.union([v.string(), v.tuple([v.string(), v.string()])]),
    ),
  ),
  dropLabels: v.pipe(
    v.optional(v.array(v.string())),
    v.description('Remove labeled statements with these label names'),
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
  clearScreen: v.pipe(
    v.optional(v.boolean()),
    v.description('Whether to clear the screen when a rebuild is triggered'),
  ),
  onInvalidate: v.pipe(
    v.optional(
      v.pipe(
        v.function(),
        v.args(v.tuple([v.string()])),
      ),
    ),
    v.description(
      'An optional function that will be called immediately every time a module changes that is part of the build.',
    ),
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
  preferBuiltinFeature: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to emit warning when detecting prefer builtin feature',
    ),
  ),
});

const CompressOptionsKeepNamesSchema = v.strictObject({
  function: v.boolean(),
  class: v.boolean(),
});

const CompressOptionsSchema = v.strictObject({
  target: v.optional(v.union([
    v.literal('esnext'),
    v.literal('es2015'),
    v.literal('es2016'),
    v.literal('es2017'),
    v.literal('es2018'),
    v.literal('es2019'),
    v.literal('es2020'),
    v.literal('es2021'),
    v.literal('es2022'),
    v.literal('es2023'),
    v.literal('es2024'),
  ])),
  dropConsole: v.optional(v.boolean()),
  dropDebugger: v.optional(v.boolean()),
  keepNames: v.optional(CompressOptionsKeepNamesSchema),
  unused: v.optional(v.union([v.boolean(), v.literal('keep_assign')])),
}) satisfies v.GenericSchema<CompressOptions>;

const MangleOptionsKeepNamesSchema = v.strictObject({
  function: v.boolean(),
  class: v.boolean(),
}) satisfies v.GenericSchema<MangleOptionsKeepNames>;

const MangleOptionsSchema = v.strictObject({
  toplevel: v.optional(v.boolean()),
  keepNames: v.optional(v.union([v.boolean(), MangleOptionsKeepNamesSchema])),
  debug: v.optional(v.boolean()),
}) satisfies v.GenericSchema<MangleOptions>;

const CodegenOptionsSchema = v.strictObject({
  removeWhitespace: v.optional(v.boolean()),
});

const MinifyOptionsSchema = v.strictObject({
  compress: v.optional(v.union([v.boolean(), CompressOptionsSchema])),
  mangle: v.optional(v.union([v.boolean(), MangleOptionsSchema])),
  codegen: v.optional(v.union([v.boolean(), CodegenOptionsSchema])),
}) satisfies v.GenericSchema<MinifyOptions>;

const ResolveOptionsSchema = v.strictObject({
  alias: v.optional(
    v.record(
      v.string(),
      v.union([v.literal(false), v.string(), v.array(v.string())]),
    ),
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
    propertyReadSideEffects: v.optional(
      v.union([v.literal(false), v.literal('always')]),
    ),
    propertyWriteSideEffects: v.optional(
      v.union([v.literal(false), v.literal('always')]),
    ),
  }),
]);

const OptimizationOptionsSchema = v.strictObject({
  inlineConst: v.pipe(
    v.optional(v.union([
      v.boolean(),
      v.strictObject({
        mode: v.optional(v.union([v.literal('all'), v.literal('smart')])),
        pass: v.optional(v.number()),
      }),
    ])),
    v.description('Enable crossmodule constant inlining'),
  ),
  pifeForModuleWrappers: v.pipe(
    v.optional(v.boolean()),
    v.description('Use PIFE pattern for module wrappers'),
  ),
}) satisfies v.GenericSchema<OptimizationOptions>;

const LogOrStringHandlerSchema = v.pipe(
  vFunction<LogOrStringHandler>(),
  v.args(v.tuple([LogLevelWithErrorSchema, RollupLogWithStringSchema])),
) satisfies v.GenericSchema<LogOrStringHandler>;

const OnLogSchema = v.pipe(
  vFunction<OnLogFunction>(),
  v.args(
    v.tuple([
      LogLevelSchema,
      RollupLogSchema,
      LogOrStringHandlerSchema,
    ]),
  ),
) satisfies v.GenericSchema<OnLogFunction>;

const OnwarnSchema = v.pipe(
  vFunction<OnwarnFunction>(),
  v.args(
    v.tuple([
      RollupLogSchema,
      v.pipe(
        vFunction(),
        v.args(
          v.tuple([
            v.union([
              RollupLogWithStringSchema,
              v.pipe(vFunction(), v.returns(RollupLogWithStringSchema)),
            ]),
          ]),
        ),
      ),
    ]),
  ),
) satisfies v.GenericSchema<OnwarnFunction>;

const HmrSchema = v.union([
  v.boolean(),
  v.strictObject({
    new: v.optional(v.boolean()),
    port: v.optional(v.number()),
    host: v.optional(v.string()),
    implement: v.optional(v.string()),
  }),
]);

const InputOptionsSchema = v.strictObject({
  input: v.optional(InputOptionSchema),
  plugins: v.optional(v.custom<RolldownPluginOption>(() => true)),
  external: v.optional(ExternalOptionSchema),
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
        styleText('underline', 'browser')
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
      `Log level (${styleText('dim', 'silent')}, ${
        styleText(['underline', 'gray'], 'info')
      }, debug, ${styleText('yellow', 'warn')})`,
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
      chunkImportMap: v.optional(v.union([
        v.boolean(),
        v.object({
          baseUrl: v.optional(v.string()),
          fileName: v.optional(v.string()),
        }),
      ])),
      nativeMagicString: v.optional(v.boolean()),
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
  tsconfig: v.pipe(
    v.optional(v.string()),
    v.description('Path to the tsconfig.json file.'),
  ),
}) satisfies v.GenericSchema<InputOptions>;

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
  preserveEntrySignatures: v.pipe(
    v.optional(v.literal(false)),
    v.description('Avoid facade chunks for entry points'),
  ),
  context: v.pipe(
    v.optional(v.string()),
    v.description('The entity top-level `this` represents.'),
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
  vFunction<AddonFunction>(),
  v.args(v.tuple([v.custom<RenderedChunk>(() => true)])),
  v.returnsAsync(
    v.unionAsync([
      v.string(),
      v.pipeAsync(v.promise(), v.awaitAsync(), v.string()),
    ]),
  ),
) satisfies v.GenericSchema<AddonFunction>;

const ChunkFileNamesFunctionSchema = v.pipe(
  vFunction<ChunkFileNamesFunction>(),
  v.args(v.tuple([v.custom<PreRenderedChunk>(() => true)])),
  v.returns(v.string()),
) satisfies v.GenericSchema<ChunkFileNamesFunction>;

const ChunkFileNamesSchema = v.union([
  v.string(),
  ChunkFileNamesFunctionSchema,
]);

const AssetFileNamesFunctionSchema = v.pipe(
  vFunction<AssetFileNamesFunction>(),
  v.args(v.tuple([v.custom<PreRenderedAsset>(() => true)])),
  v.returns(v.string()),
) satisfies v.GenericSchema<AssetFileNamesFunction>;

const AssetFileNamesSchema = v.union([
  v.string(),
  AssetFileNamesFunctionSchema,
]);

const SanitizeFileNameFunctionSchema = v.pipe(
  vFunction<SanitizeFileNameFunction>(),
  v.args(v.tuple([v.string()])),
  v.returns(v.string()),
) satisfies v.GenericSchema<SanitizeFileNameFunction>;

const SanitizeFileNameSchema = v.union([
  v.boolean(),
  SanitizeFileNameFunctionSchema,
]);

const GlobalsFunctionSchema = v.pipe(
  vFunction<GlobalsFunction>(),
  v.args(v.tuple([v.string()])),
  v.returns(v.string()),
) satisfies v.GenericSchema<GlobalsFunction>;

const PathsFunctionSchema = v.pipe(
  vFunction<PathsFunction>(),
  v.args(v.tuple([v.string()])),
  v.returns(v.string()),
) satisfies v.GenericSchema<PathsFunction>;

const ManualChunksFunctionSchema = v.pipe(
  vFunction<ManualChunksFunction>(),
  v.args(v.tuple([v.string(), v.object({})])),
  v.returns(v.nullish(v.string())),
) satisfies v.GenericSchema<ManualChunksFunction>;

const AdvancedChunksNameFunctionSchema = v.pipe(
  vFunction<AdvancedChunksNameFunction>(),
  v.args(v.tuple([v.string(), v.object({})])),
  v.returns(v.nullish(v.string())),
) satisfies v.GenericSchema<AdvancedChunksNameFunction>;

const AdvancedChunksTestFunctionSchema = v.pipe(
  vFunction<AdvancedChunksTestFunction>(),
  v.args(v.tuple([v.string()])),
  v.returns(v.union([v.boolean(), v.void(), v.undefined()])),
) satisfies v.GenericSchema<AdvancedChunksTestFunction>;

const AdvancedChunksSchema = v.strictObject({
  includeDependenciesRecursively: v.optional(v.boolean()),
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
          AdvancedChunksNameFunctionSchema,
        ]),
        test: v.optional(
          v.union([
            StringOrRegExpSchema,
            AdvancedChunksTestFunctionSchema,
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

const GeneratedCodePresetSchema = v.union([
  v.literal('es5'),
  v.literal('es2015'),
]);

const GeneratedCodeOptionsSchema = v.strictObject({
  symbols: v.pipe(
    v.optional(v.boolean()),
    v.description('Whether to use Symbol.toStringTag for namespace objects'),
  ),
  preset: GeneratedCodePresetSchema,
  profilerNames: v.pipe(
    v.optional(v.boolean()),
    v.description(
      'Whether to add readable names to internal variables for profiling purposes',
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
        styleText('underline', 'auto')
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
        styleText('underline', 'esm')
      }, cjs, and iife)`,
    ),
  ),
  sourcemap: v.pipe(
    v.optional(
      v.union([v.boolean(), v.literal('inline'), v.literal('hidden')]),
    ),
    v.description(
      `Generate sourcemap (\`-s inline\` for inline, or ${
        styleText(
          'bold',
          'pass the `-s` on the last argument if you want to generate `.map` file',
        )
      })`,
    ),
  ),
  sourcemapBaseUrl: v.pipe(
    v.optional(v.string()),
    v.description('Base URL used to prefix sourcemap paths'),
  ),
  sourcemapDebugIds: v.pipe(
    v.optional(v.boolean()),
    v.description('Inject sourcemap debug IDs'),
  ),
  sourcemapIgnoreList: v.optional(
    v.union([
      v.boolean(),
      v.custom<SourcemapIgnoreListOption>(() => true),
      StringOrRegExpSchema,
    ]),
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
      v.union([v.boolean(), v.literal('dce-only'), MinifyOptionsSchema]),
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
  paths: v.pipe(
    v.optional(
      v.union([v.record(v.string(), v.string()), PathsFunctionSchema]),
    ),
    v.description(
      'Maps external module IDs to paths',
    ),
  ),
  generatedCode: v.pipe(
    v.optional(
      v.partial(GeneratedCodeOptionsSchema),
    ),
    v.description('Generated code options'),
  ),
  externalLiveBindings: v.pipe(
    v.optional(v.boolean()),
    v.description('external live bindings'),
  ),
  inlineDynamicImports: v.pipe(
    v.optional(v.boolean()),
    v.description('Inline dynamic imports'),
  ),
  manualChunks: v.optional(ManualChunksFunctionSchema),
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
  hoistTransitiveImports: v.optional(v.literal(false)),
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
  topLevelVar: v.pipe(
    v.optional(v.boolean()),
    v.description('Rewrite top-level declarations to use `var`.'),
  ),
  cleanDir: v.pipe(
    v.optional(v.boolean()),
    v.description('Clean output directory before emitting output'),
  ),
  keepNames: v.pipe(
    v.optional(v.boolean()),
    v.description('Keep function and class names after bundling'),
  ),
}) satisfies v.GenericSchema<OutputOptions>;

const getAddonDescription = (
  placement: 'bottom' | 'top',
  wrapper: 'inside' | 'outside',
) => {
  return `Code to insert the ${
    styleText('bold', placement)
  } of the bundled file (${styleText('bold', wrapper)} the wrapper function)`;
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
  environment: v.pipe(
    v.optional(v.union([v.string(), v.array(v.string())])),
    v.description(
      'Pass additional settings to the config file via process.ENV.',
    ),
  ),
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

type HelperMsgRecord = Record<
  string,
  { ignored?: boolean; issueMsg?: string; help?: string }
>;
const inputHelperMsgRecord: HelperMsgRecord = {
  output: { ignored: true },
  'resolve.tsconfigFilename': {
    issueMsg:
      'It is deprecated. Please use the top-level `tsconfig` option instead.',
  },
};
const outputHelperMsgRecord: HelperMsgRecord = {};

export function validateOption<T>(key: 'input' | 'output', options: T): void {
  if (typeof options !== 'object') {
    throw new Error(
      `Invalid ${key} options. Expected an Object but received ${
        JSON.stringify(options)
      }.`,
    );
  }

  if (globalThis.process?.env?.ROLLUP_TEST) return;
  let parsed = v.safeParse(
    key === 'input' ? InputOptionsSchema : OutputOptionsSchema,
    options,
  );

  if (!parsed.success) {
    const errors = parsed.issues
      .map((issue) => {
        let issueMsg = issue.message;
        const issuePaths = issue.path!.map((path) => path.key);
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
        return `- For the "${stringPath}". ${
          helper?.issueMsg ||
          issueMsg + '.'
        } ${helper?.help ? `\n  Help: ${helper.help}` : ''}`;
      })
      .filter(Boolean);
    if (errors.length) {
      console.warn(
        `\x1b[33mWarning: Invalid ${key} options (${errors.length} issue${
          errors.length === 1 ? '' : 's'
        } found)\n${errors.join('\n')}\x1b[0m`,
      );
    }
  }
}

export function getInputCliKeys(): string[] {
  return v.keyof(InputCliOptionsSchema).options;
}

export function getOutputCliKeys(): string[] {
  return v.keyof(OutputCliOptionsSchema).options;
}

export function getCliSchemaInfo(): Record<
  string,
  { type: string; description?: string }
> {
  return flattenValibotSchema(CliOptionsSchema);
}
