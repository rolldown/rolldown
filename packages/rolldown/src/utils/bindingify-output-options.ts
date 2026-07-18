import type { BindingChunkingContext, BindingOutputOptions } from '../binding.cjs';
import type { OutputOptions } from '../options/output-options';
import type { BuildCallbackRunner } from '../plugin/bindingify-plugin';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { ChunkingContextImpl } from '../types/chunking-context';
import { transformAssetSource } from './asset-source';
import { unimplemented } from './misc';
import { transformRenderedChunk } from './transform-rendered-chunk';
import { logger } from '../cli/logger';

export function bindingifyOutputOptions(
  outputOptions: OutputOptions,
  pluginContextData: PluginContextData,
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
    sourcemapFileNames,
    sourcemapExcludeSources,
    sourcemapIgnoreList: wrapOptionalBuildCallback(
      sourcemapIgnoreList ?? /node_modules/,
      runBuildCallback,
    ),
    sourcemapPathTransform: wrapOptionalBuildCallback(sourcemapPathTransform, runBuildCallback),
    banner: bindingifyAddon(banner, runBuildCallback),
    footer: bindingifyAddon(footer, runBuildCallback),
    postBanner: bindingifyAddon(postBanner, runBuildCallback),
    postFooter: bindingifyAddon(postFooter, runBuildCallback),
    intro: bindingifyAddon(intro, runBuildCallback),
    outro: bindingifyAddon(outro, runBuildCallback),
    extend: outputOptions.extend,
    globals: wrapOptionalBuildCallback(globals, runBuildCallback),
    paths: wrapOptionalBuildCallback(paths, runBuildCallback),
    generatedCode,
    esModule,
    name,
    assetFileNames: bindingifyAssetFilenames(assetFileNames, runBuildCallback),
    entryFileNames: wrapOptionalBuildCallback(entryFileNames, runBuildCallback),
    chunkFileNames: wrapOptionalBuildCallback(chunkFileNames, runBuildCallback),
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
  runBuildCallback?: BuildCallbackRunner,
): BindingOutputOptions[AddonKeys] {
  if (configAddon == null || configAddon === '') {
    return undefined;
  }
  if (typeof configAddon === 'function') {
    return async (chunk) =>
      runBuildCallback
        ? runBuildCallback(() => configAddon(transformRenderedChunk(chunk)))
        : configAddon(transformRenderedChunk(chunk));
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
        const { name, ...restGroup } = group;
        return {
          ...restGroup,
          test: wrapOptionalBuildCallback(restGroup.test, runBuildCallback),
          name:
            typeof name === 'function'
              ? (id: string, ctx: BindingChunkingContext) =>
                  runBuildCallback
                    ? runBuildCallback(() =>
                        name(id, new ChunkingContextImpl(ctx, pluginContextData)),
                      )
                    : name(id, new ChunkingContextImpl(ctx, pluginContextData))
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

function wrapOptionalBuildCallback<Value>(
  value: Value,
  runBuildCallback?: BuildCallbackRunner,
): Value {
  if (!runBuildCallback || typeof value !== 'function') return value;
  const callback = value as (...args: unknown[]) => unknown;
  return ((...args: unknown[]) => runBuildCallback(() => callback(...args))) as Value;
}
