import type { BindingOutputOptions } from '../binding';
import type { OutputOptions } from '../options/output-options';
import { ChunkingContextImpl } from '../types/chunking-context';
import { transformAssetSource } from './asset-source';
import { unimplemented } from './misc';
import { transformRenderedChunk } from './transform-rendered-chunk';

export function bindingifyOutputOptions(
  outputOptions: OutputOptions,
): BindingOutputOptions {
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
    emptyOutDir,
  } = outputOptions;

  const advancedChunks = bindingifyAdvancedChunks(
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
    inlineDynamicImports: outputOptions.inlineDynamicImports,
    advancedChunks,
    polyfillRequire: outputOptions.polyfillRequire,
    sanitizeFileName,
    preserveModules,
    virtualDirname,
    legalComments,
    preserveModulesRoot,
    topLevelVar,
    minifyInternalExports: outputOptions.minifyInternalExports,
    emptyOutDir: emptyOutDir,
  };
}

type AddonKeys = 'banner' | 'footer' | 'intro' | 'outro';

function bindingifyAddon(
  configAddon: OutputOptions[AddonKeys],
): BindingOutputOptions[AddonKeys] {
  return async (chunk) => {
    if (typeof configAddon === 'function') {
      return configAddon(transformRenderedChunk(chunk));
    }
    return configAddon || '';
  };
}

function bindingifyFormat(
  format: OutputOptions['format'],
): BindingOutputOptions['format'] {
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

function bindingifyAdvancedChunks(
  advancedChunks: OutputOptions['advancedChunks'],
  manualChunks: OutputOptions['manualChunks'],
): BindingOutputOptions['advancedChunks'] {
  if (manualChunks != null && advancedChunks != null) {
    console.warn(
      '`manualChunks` option is ignored due to `advancedChunks` option is specified.',
    );
  } else if (manualChunks != null) {
    advancedChunks = {
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

  if (advancedChunks == null) {
    return undefined;
  }

  const { groups, ...restAdvancedChunks } = advancedChunks;

  return {
    ...restAdvancedChunks,
    groups: groups?.map((group) => {
      const { name, ...restGroup } = group;

      return {
        ...restGroup,
        name: typeof name === 'function'
          ? (id, ctx) => name(id, new ChunkingContextImpl(ctx))
          : name,
      };
    }),
  };
}
