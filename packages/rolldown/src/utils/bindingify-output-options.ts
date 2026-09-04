import type { BindingChunkingContext, BindingOutputOptions } from '../binding.cjs';
import type {
  CodeSplittingNameFunction,
  CodeSplittingTestFunction,
  OutputOptions,
} from '../options/output-options';
import type { BuildCallbackRunner } from '../plugin/bindingify-plugin';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { ChunkingContextImpl } from '../types/chunking-context';
import { transformAssetSource } from './asset-source';
import { unimplemented } from './misc';
import { releaseOrDefer, shouldEagerlyFreeOutputs } from './threadless-free';
import { snapshotRenderedChunk, transformRenderedChunk } from './transform-rendered-chunk';
import { logger } from '../cli/logger';
import {
  measureHookCost,
  measureIfFunction,
  OUTPUT_OPTIONS_OWNER,
  type PluginTimingsRecorder,
} from './plugin-timings';

export function bindingifyOutputOptions(
  outputOptions: OutputOptions,
  pluginContextData: PluginContextData,
  timings: PluginTimingsRecorder | undefined,
  runBuildCallback?: BuildCallbackRunner,
): BindingOutputOptions {
  const {
    dir,
    format,
    exports,
    hashCharacters,
    sourcemap,
    sourcemapBaseUrl,
    sourcemapDebugIds,
    sourcemapFileNames,
    sourcemapExcludeSources,
    sourcemapIgnoreList,
    sourcemapPathTransform,
    name,
    assetFileNames,
    entryFileNames,
    chunkFileNames,
    banner,
    footer,
    postBanner,
    postFooter,
    intro,
    outro,
    esModule,
    globals,
    paths,
    generatedCode,
    file,
    // Already measured at the source; see `createBundlerOptions`.
    sanitizeFileName,
    preserveModules,
    virtualDirname,
    legalComments,
    comments,
    preserveModulesRoot,
    manualChunks,
    topLevelVar,
    cleanDir,
    strictExecutionOrder,
  } = outputOptions;

  if (legalComments != null) {
    logger.warn('`legalComments` option is deprecated, please use `comments.legal` instead.');
  }

  // Handle codeSplitting and inlineDynamicImports
  const { inlineDynamicImports, advancedChunks } = bindingifyCodeSplitting(
    outputOptions.codeSplitting,
    outputOptions.inlineDynamicImports,
    outputOptions.advancedChunks,
    manualChunks,
    pluginContextData,
    timings,
    runBuildCallback,
  );

  return {
    dir,
    // Handle case: rollup/test/sourcemaps/samples/sourcemap-file-hashed/_config.js
    file: file == null ? undefined : file,
    format: bindingifyFormat(format),
    exports,
    hashCharacters,
    sourcemap: bindingifySourcemap(sourcemap),
    sourcemapBaseUrl,
    sourcemapDebugIds,
    sourcemapFileNames: wrapOptionalBuildCallback(
      measureIfFunction(timings, OUTPUT_OPTIONS_OWNER, 'sourcemapFileNames', sourcemapFileNames),
      runBuildCallback,
    ),
    sourcemapExcludeSources,
    sourcemapIgnoreList: wrapOptionalBuildCallback(
      measureIfFunction(
        timings,
        OUTPUT_OPTIONS_OWNER,
        'sourcemapIgnoreList',
        sourcemapIgnoreList ?? /node_modules/,
      ),
      runBuildCallback,
    ),
    sourcemapPathTransform: wrapOptionalBuildCallback(
      measureIfFunction(
        timings,
        OUTPUT_OPTIONS_OWNER,
        'sourcemapPathTransform',
        sourcemapPathTransform,
      ),
      runBuildCallback,
    ),
    banner: bindingifyAddon(banner, 'banner', timings, runBuildCallback),
    footer: bindingifyAddon(footer, 'footer', timings, runBuildCallback),
    postBanner: bindingifyAddon(postBanner, 'postBanner', timings, runBuildCallback),
    postFooter: bindingifyAddon(postFooter, 'postFooter', timings, runBuildCallback),
    intro: bindingifyAddon(intro, 'intro', timings, runBuildCallback),
    outro: bindingifyAddon(outro, 'outro', timings, runBuildCallback),
    extend: outputOptions.extend,
    globals: wrapOptionalBuildCallback(
      measureIfFunction(timings, OUTPUT_OPTIONS_OWNER, 'globals', globals),
      runBuildCallback,
    ),
    paths: wrapOptionalBuildCallback(
      measureIfFunction(timings, OUTPUT_OPTIONS_OWNER, 'paths', paths),
      runBuildCallback,
    ),
    generatedCode,
    esModule,
    name,
    // Already measured at the source; see `createBundlerOptions`.
    assetFileNames: bindingifyAssetFilenames(assetFileNames, runBuildCallback),
    entryFileNames: wrapOptionalBuildCallback(
      measureIfFunction(timings, OUTPUT_OPTIONS_OWNER, 'entryFileNames', entryFileNames),
      runBuildCallback,
    ),
    chunkFileNames: wrapOptionalBuildCallback(
      measureIfFunction(timings, OUTPUT_OPTIONS_OWNER, 'chunkFileNames', chunkFileNames),
      runBuildCallback,
    ),
    // TODO(sapphi-red): support parallel plugins
    plugins: [],
    minify: outputOptions.minify,
    externalLiveBindings: outputOptions.externalLiveBindings,
    inlineDynamicImports,
    dynamicImportInCjs: outputOptions.dynamicImportInCjs,
    manualCodeSplitting: advancedChunks,
    polyfillRequire: outputOptions.polyfillRequire,
    sanitizeFileName: wrapOptionalBuildCallback(sanitizeFileName, runBuildCallback),
    preserveModules,
    virtualDirname,
    legalComments,
    comments: bindingifyComments(comments),
    preserveModulesRoot,
    topLevelVar,
    minifyInternalExports: outputOptions.minifyInternalExports,
    cleanDir,
    strictExecutionOrder,
    strict: outputOptions.strict,
  };
}

type AddonKeys = 'banner' | 'footer' | 'postBanner' | 'postFooter' | 'intro' | 'outro';

function bindingifyAddon(
  configAddon: OutputOptions[AddonKeys],
  name: AddonKeys,
  timings: PluginTimingsRecorder | undefined,
  runBuildCallback?: BuildCallbackRunner,
): BindingOutputOptions[AddonKeys] {
  if (configAddon == null || configAddon === '') {
    return undefined;
  }
  if (typeof configAddon === 'function') {
    // Measure the user's callback, not `transformRenderedChunk` around it, so the row is
    // their work rather than the conversion their choice of a function forced.
    const measured = measureHookCost(timings, OUTPUT_OPTIONS_OWNER, name, configAddon);
    return async (chunk) => {
      // On the threadless flavor the per-invocation chunk box would otherwise
      // wait for GC finalizers that never run there; hand the callback a
      // plain-data snapshot and release the box up front.
      const rendered = shouldEagerlyFreeOutputs()
        ? snapshotRenderedChunk(chunk)
        : transformRenderedChunk(chunk);
      return runBuildCallback ? runBuildCallback(() => measured(rendered)) : measured(rendered);
    };
  }
  return configAddon;
}

function bindingifyFormat(format: OutputOptions['format']): BindingOutputOptions['format'] {
  switch (format) {
    case undefined:
    case 'es':
    case 'esm':
    case 'module': {
      return 'es';
    }
    case 'cjs':
    case 'commonjs': {
      return 'cjs';
    }
    case 'iife': {
      return 'iife';
    }
    case 'umd': {
      return 'umd';
    }
    default:
      unimplemented(`output.format: ${format}`);
  }
}

function bindingifySourcemap(
  sourcemap: OutputOptions['sourcemap'],
): BindingOutputOptions['sourcemap'] {
  switch (sourcemap) {
    case true:
      return 'file';
    case 'inline':
      return 'inline';
    case false:
    case undefined:
      return undefined;
    case 'hidden':
      return 'hidden';
    default:
      throw new Error(`unknown sourcemap: ${sourcemap}`);
  }
}

function bindingifyAssetFilenames(
  assetFileNames: OutputOptions['assetFileNames'],
  runBuildCallback?: BuildCallbackRunner,
): BindingOutputOptions['assetFileNames'] {
  if (typeof assetFileNames === 'function') {
    return (asset) => {
      const invoke = () =>
        assetFileNames({
          name: asset.name,
          names: asset.names,
          originalFileName: asset.originalFileName,
          originalFileNames: asset.originalFileNames,
          source: transformAssetSource(asset.source),
          type: 'asset',
        });
      return runBuildCallback ? runBuildCallback(invoke) : invoke();
    };
  }
  return assetFileNames;
}

function bindingifyComments(comments: OutputOptions['comments']): BindingOutputOptions['comments'] {
  if (comments == null) {
    return undefined;
  }
  if (typeof comments === 'boolean') {
    return comments;
  }
  return comments;
}

function bindingifyCodeSplitting(
  codeSplitting: OutputOptions['codeSplitting'],
  inlineDynamicImportsOption: OutputOptions['inlineDynamicImports'],
  advancedChunks: OutputOptions['advancedChunks'],
  manualChunks: OutputOptions['manualChunks'],
  pluginContextData: PluginContextData,
  timings: PluginTimingsRecorder | undefined,
  runBuildCallback?: BuildCallbackRunner,
): {
  inlineDynamicImports: BindingOutputOptions['inlineDynamicImports'];
  advancedChunks: BindingOutputOptions['manualCodeSplitting'];
} {
  let inlineDynamicImports: boolean | undefined;
  let effectiveChunksOption: Exclude<OutputOptions['codeSplitting'], boolean> | undefined;

  // Handle codeSplitting boolean values
  if (codeSplitting === false) {
    // Warn if inlineDynamicImports is also set
    if (inlineDynamicImportsOption != null) {
      logger.warn(
        '`inlineDynamicImports` option is ignored because `codeSplitting: false` is set.',
      );
    }
    // Validate that manualChunks is not set with code splitting disabled
    if (manualChunks != null) {
      throw new Error(
        'Invalid configuration: "output.manualChunks" cannot be used when "output.codeSplitting" is set to false.',
      );
    }
    // When code splitting is disabled, ignore advancedChunks
    if (advancedChunks != null) {
      logger.warn('`advancedChunks` option is ignored because `codeSplitting` is set to `false`.');
    }
    // Return early - no advanced chunks when code splitting is disabled
    return {
      inlineDynamicImports: true,
      advancedChunks: undefined,
    };
  } else if (codeSplitting === true) {
    // Explicit code splitting enabled - ignore deprecated inlineDynamicImports
    if (inlineDynamicImportsOption != null) {
      logger.warn('`inlineDynamicImports` option is ignored because `codeSplitting: true` is set.');
    }
  } else if (codeSplitting == null) {
    // Default behavior: no inlining, automatic code splitting
    // Check if deprecated inlineDynamicImports is used
    if (inlineDynamicImportsOption != null) {
      logger.warn(
        '`inlineDynamicImports` option is deprecated, please use `codeSplitting: false` instead.',
      );
      inlineDynamicImports = inlineDynamicImportsOption;
    }
  } else {
    // codeSplitting is an object (advanced config)
    effectiveChunksOption = codeSplitting;
    // Ignore inlineDynamicImports if codeSplitting object is specified
    if (inlineDynamicImportsOption != null) {
      logger.warn(
        '`inlineDynamicImports` option is ignored because the `codeSplitting` option is specified.',
      );
    }
  }

  // Validate inlineDynamicImports conflicts with manualChunks
  if (inlineDynamicImports === true && manualChunks != null) {
    throw new Error(
      'Invalid value "true" for option "output.inlineDynamicImports" - this option is not supported for "output.manualChunks".',
    );
  }

  // Handle advancedChunks deprecation (only if codeSplitting is not set to object)
  if (effectiveChunksOption == null) {
    if (advancedChunks != null) {
      logger.warn('`advancedChunks` option is deprecated, please use `codeSplitting` instead.');
      effectiveChunksOption = advancedChunks;
    }
  } else if (advancedChunks != null) {
    logger.warn(
      '`advancedChunks` option is ignored because the `codeSplitting` option is specified.',
    );
  }

  // Handle manualChunks migration
  if (manualChunks != null && effectiveChunksOption != null) {
    logger.warn(
      '`manualChunks` option is ignored because the `codeSplitting` option is specified.',
    );
  } else if (manualChunks != null) {
    effectiveChunksOption = {
      groups: [
        {
          name(moduleId, ctx) {
            return manualChunks(moduleId, {
              getModuleInfo: (id) => ctx.getModuleInfo(id),
            });
          },
        },
      ],
    };
  }

  // `inlineDynamicImports: true` (the deprecated alias for `codeSplitting: false`) disables code
  // splitting, so any resolved chunk grouping is dropped here, mirroring the `codeSplitting: false`
  // path above. `manualChunks` already throws earlier, so only `advancedChunks` can reach this
  // point. Without this, the grouping would be forwarded and then silently discarded in the Rust
  // binding, ignoring the requested groups without any diagnostic.
  if (inlineDynamicImports === true && effectiveChunksOption != null) {
    logger.warn(
      '`advancedChunks` option is ignored because `inlineDynamicImports: true` disables code splitting.',
    );
    effectiveChunksOption = undefined;
  }

  // Transform effectiveChunksOption to binding format
  let advancedChunksResult: BindingOutputOptions['manualCodeSplitting'];
  if (effectiveChunksOption != null) {
    const { groups, ...restOptions } = effectiveChunksOption;
    advancedChunksResult = {
      ...restOptions,
      groups: groups?.map((group) => {
        const { name, test, ...restGroup } = group;
        return {
          ...restGroup,
          test: wrapOptionalBuildCallback(
            typeof test === 'function'
              ? batchTest(
                  measureHookCost(
                    timings,
                    OUTPUT_OPTIONS_OWNER,
                    'codeSplitting groups[].test',
                    test,
                  ),
                )
              : test,
            runBuildCallback,
          ),
          // The core calls this classifier directly rather than through a plugin, so it
          // belongs to no plugin's rows — and it runs once per module, which is how it ends
          // up dominating a build.
          name:
            typeof name === 'function'
              ? wrapOptionalBuildCallback(
                  batchName(
                    measureHookCost(
                      timings,
                      OUTPUT_OPTIONS_OWNER,
                      'codeSplitting groups[].name',
                      name,
                    ),
                    pluginContextData,
                  ),
                  runBuildCallback,
                )
              : name,
        };
      }),
    };
  }

  return {
    inlineDynamicImports,
    advancedChunks: advancedChunksResult,
  };
}

/**
 * Wraps a per-id `test` in the batched shim that the binding expects.
 *
 * The loop runs in JS so that a group makes one napi crossing, not one per module.
 *
 * The result is a `Uint8Array`, which crosses as a buffer instead of one tagged value per id.
 */
function batchTest(test: CodeSplittingTestFunction): (ids: string[]) => Uint8Array {
  return (ids) => {
    const results = new Uint8Array(ids.length);
    for (let index = 0; index < ids.length; index++) {
      const result = test(ids[index]);
      // napi reports the type of the array, not of the bad element, so the check runs here.
      if (result != null && typeof result !== 'boolean') {
        throw new TypeError(
          `\`output.codeSplitting.groups[].test\` returned ${typeof result} for module "${
            ids[index]
          }", but expected a boolean, null or undefined.`,
        );
      }
      results[index] = result === true ? 1 : 0;
    }
    return results;
  };
}

/**
 * This is the `name` equivalent of {@linkcode batchTest}. The context wrapper holds no per-call
 * state, so one instance serves the whole batch.
 */
function batchName(
  name: CodeSplittingNameFunction,
  pluginContextData: PluginContextData,
): (
  ids: string[],
  bindingContext: BindingChunkingContext,
) => ReturnType<CodeSplittingNameFunction>[] {
  return (ids, bindingContext) => {
    const context = new ChunkingContextImpl(bindingContext, pluginContextData);
    const results: ReturnType<CodeSplittingNameFunction>[] = [];
    // The classifier is sync by contract (the binding wants plain strings
    // back), so the batch's context box can be released as soon as the loop
    // finishes — after the last candidate's `name` call, its final native
    // read. Each batch invocation gets a freshly minted box, so releasing it
    // here cannot affect later groups. Any getModuleInfo boxes the callbacks
    // minted were already snapshot-and-dropped by `ChunkingContextImpl`.
    try {
      for (let index = 0; index < ids.length; index++) {
        const result = name(ids[index], context);
        // napi reports the type of the array, not of the bad element, so the check runs here.
        if (result != null && typeof result !== 'string') {
          throw new TypeError(
            `\`output.codeSplitting.groups[].name\` returned ${typeof result} for module "${
              ids[index]
            }", but expected a string, null or undefined.`,
          );
        }
        results.push(result);
      }
    } finally {
      releaseOrDefer(bindingContext);
    }
    return results;
  };
}

function wrapOptionalBuildCallback<Value>(
  value: Value,
  runBuildCallback?: BuildCallbackRunner,
): Value {
  if (!runBuildCallback || typeof value !== 'function') return value;
  const callback = value as (...args: unknown[]) => unknown;
  return ((...args: unknown[]) => runBuildCallback(() => callback(...args))) as Value;
}
