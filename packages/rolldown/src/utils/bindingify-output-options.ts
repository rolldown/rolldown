import type { BindingChunkingContext, BindingOutputOptions } from '../binding.cjs';
import type { OutputOptions } from '../options/output-options';
import { ChunkingContextImpl } from '../types/chunking-context';
import { transformAssetSource } from './asset-source';
import { unimplemented } from './misc';
import { transformRenderedChunk } from './transform-rendered-chunk';
import { logger } from '../cli/logger';

export function bindingifyOutputOptions(outputOptions: OutputOptions): BindingOutputOptions {
  const {
    dir,
    format,
    exports,
    hashCharacters,
    sourcemap,
    sourcemapBaseUrl,
    sourcemapDebugIds,
    sourcemapIgnoreList,
    sourcemapPathTransform,
    name,
    assetFileNames,
    entryFileNames,
    chunkFileNames,
    cssEntryFileNames,
    cssChunkFileNames,
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
    preserveModulesRoot,
    manualChunks,
    topLevelVar,
    cleanDir,
    strictExecutionOrder,
  } = outputOptions;

  // Handle codeSplitting and inlineDynamicImports
  const { inlineDynamicImports, advancedChunks } = bindingifyCodeSplitting(
    outputOptions.codeSplitting,
    outputOptions.inlineDynamicImports,
    outputOptions.advancedChunks,
    manualChunks,
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
    sourcemapIgnoreList: sourcemapIgnoreList ?? /node_modules/,
    sourcemapPathTransform,
    banner: bindingifyAddon(banner),
    footer: bindingifyAddon(footer),
    postBanner: bindingifyAddon(postBanner),
    postFooter: bindingifyAddon(postFooter),
    intro: bindingifyAddon(intro),
    outro: bindingifyAddon(outro),
    extend: outputOptions.extend,
    globals,
    paths,
    generatedCode,
    esModule,
    name,
    assetFileNames: bindingifyAssetFilenames(assetFileNames),
    entryFileNames,
    chunkFileNames,
    cssEntryFileNames,
    cssChunkFileNames,
    // TODO(sapphi-red): support parallel plugins
    plugins: [],
    minify: outputOptions.minify,
    externalLiveBindings: outputOptions.externalLiveBindings,
    inlineDynamicImports,
    dynamicImportInCjs: outputOptions.dynamicImportInCjs,
    manualCodeSplitting: advancedChunks,
    polyfillRequire: outputOptions.polyfillRequire,
    sanitizeFileName,
    preserveModules,
    virtualDirname,
    legalComments,
    preserveModulesRoot,
    topLevelVar,
    minifyInternalExports: outputOptions.minifyInternalExports,
    cleanDir,
    strictExecutionOrder,
  };
}

type AddonKeys = 'banner' | 'footer' | 'intro' | 'outro';

function bindingifyAddon(configAddon: OutputOptions[AddonKeys]): BindingOutputOptions[AddonKeys] {
  if (configAddon == null || configAddon === '') {
    return undefined;
  }
  if (typeof configAddon === 'function') {
    return async (chunk) => configAddon(transformRenderedChunk(chunk));
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
): BindingOutputOptions['assetFileNames'] {
  if (typeof assetFileNames === 'function') {
    return (asset) => {
      return assetFileNames({
        name: asset.name,
        names: asset.names,
        originalFileName: asset.originalFileName,
        originalFileNames: asset.originalFileNames,
        source: transformAssetSource(asset.source),
        type: 'asset',
      });
    };
  }
  return assetFileNames;
}

function bindingifyCodeSplitting(
  codeSplitting: OutputOptions['codeSplitting'],
  inlineDynamicImportsOption: OutputOptions['inlineDynamicImports'],
  advancedChunks: OutputOptions['advancedChunks'],
  manualChunks: OutputOptions['manualChunks'],
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
          name:
            typeof name === 'function'
              ? (id: string, ctx: BindingChunkingContext) => name(id, new ChunkingContextImpl(ctx))
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
