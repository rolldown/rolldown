import type { BindingOutputOptions } from '../binding';
import type { OutputOptions } from '../options/output-options';
import type { SourcemapIgnoreListOption } from '../types/misc';
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
    file,
    sanitizeFileName,
    preserveModules,
    virtualDirname,
    legalComments,
    preserveModulesRoot
  } = outputOptions;

  return {
    dir,
    // Handle case: rollup/test/sourcemaps/samples/sourcemap-file-hashed/_config.js
    file: file == null ? undefined : file,
    format: bindingifyFormat(format),
    exports,
    hashCharacters,
    sourcemap: bindingifySourcemap(sourcemap),
    sourcemapDebugIds,
    sourcemapIgnoreList: bindingifySourcemapIgnoreList(sourcemapIgnoreList),
    sourcemapPathTransform,
    banner: bindingifyAddon(banner),
    footer: bindingifyAddon(footer),
    intro: bindingifyAddon(intro),
    outro: bindingifyAddon(outro),
    extend: outputOptions.extend,
    globals,
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
    advancedChunks: outputOptions.advancedChunks,
    polyfillRequire: outputOptions.polyfillRequire,
    target: outputOptions.target,
    sanitizeFileName,
    preserveModules,
    virtualDirname,
    legalComments,
    preserveModulesRoot
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
    case 'experimental-app': {
      return 'app';
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

export function bindingifySourcemapIgnoreList(
  sourcemapIgnoreList: OutputOptions['sourcemapIgnoreList'],
): SourcemapIgnoreListOption {
  return typeof sourcemapIgnoreList === 'function'
    ? sourcemapIgnoreList
    : sourcemapIgnoreList === false
    ? () => false
    : (relativeSourcePath: string, _sourcemapPath: string) =>
      relativeSourcePath.includes('node_modules');
}

function bindingifyAssetFilenames(
  assetFileNames: OutputOptions['assetFileNames'],
): BindingOutputOptions['assetFileNames'] {
  if (typeof assetFileNames === 'function') {
    return (asset) => {
      return assetFileNames({
        names: asset.names,
        originalFileNames: asset.originalFileNames,
        source: transformAssetSource(asset.source),
        type: 'asset',
      });
    };
  }
  return assetFileNames;
}
