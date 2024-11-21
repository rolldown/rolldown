import type { OutputOptions } from '../types/output-options'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'

export function normalizeOutputOptions(
  opts: OutputOptions,
): NormalizedOutputOptions {
  const {
    dir,
    format,
    exports,
    hashCharacters,
    sourcemap,
    sourcemapIgnoreList,
    sourcemapPathTransform,
    minify,
    extend,
    globals,
    assetFileNames,
    entryFileNames,
    chunkFileNames,
    cssEntryFileNames,
    cssChunkFileNames,
    name,
    esModule,
    file,
    externalLiveBindings,
    inlineDynamicImports,
    advancedChunks,
  } = opts
  return {
    dir,
    file,
    format,
    exports,
    hashCharacters,
    sourcemap,
    sourcemapIgnoreList:
      typeof sourcemapIgnoreList === 'function'
        ? sourcemapIgnoreList
        : sourcemapIgnoreList === false
          ? () => false
          : (relativeSourcePath: string, _sourcemapPath: string) =>
              relativeSourcePath.includes('node_modules'),
    sourcemapPathTransform,
    banner: getAddon(opts, 'banner'),
    footer: getAddon(opts, 'footer'),
    intro: getAddon(opts, 'intro'),
    outro: getAddon(opts, 'outro'),
    esModule,
    // TODO support functions
    globals,
    entryFileNames,
    chunkFileNames,
    cssEntryFileNames,
    cssChunkFileNames,
    assetFileNames,
    plugins: [],
    minify,
    extend,
    name,
    externalLiveBindings,
    inlineDynamicImports,
    advancedChunks,
  }
}

const getAddon = <T extends 'banner' | 'footer' | 'intro' | 'outro'>(
  config: OutputOptions,
  name: T,
): NormalizedOutputOptions[T] => {
  return async (chunk) => {
    const configAddon = config[name]
    if (typeof configAddon === 'function') {
      return configAddon(chunk)
    }
    return configAddon || ''
  }
}
