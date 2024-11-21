import { unimplemented } from '../utils/misc'
import type { BindingOutputOptions } from '../binding'
import type { NormalizedOutputOptions } from './normalized-output-options'

export function bindingifyOutputOptions(
  outputOptions: NormalizedOutputOptions,
): BindingOutputOptions {
  const {
    dir,
    format,
    exports,
    hashCharacters,
    sourcemap,
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
  } = outputOptions
  return {
    dir,
    // Handle case: rollup/test/sourcemaps/samples/sourcemap-file-hashed/_config.js
    file: file == null ? undefined : file,
    format: bindingifyFormat(format),
    exports,
    hashCharacters,
    sourcemap: bindingifySourcemap(sourcemap),
    sourcemapIgnoreList,
    sourcemapPathTransform,
    banner,
    footer,
    intro,
    outro,
    extend: outputOptions.extend,
    globals,
    esModule,
    name,
    assetFileNames,
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
  }
}

function bindingifyFormat(
  format: NormalizedOutputOptions['format'],
): BindingOutputOptions['format'] {
  switch (format) {
    case undefined:
    case 'es':
    case 'esm':
    case 'module': {
      return 'es'
    }
    case 'cjs':
    case 'commonjs': {
      return 'cjs'
    }
    case 'iife': {
      return 'iife'
    }
    case 'umd': {
      return 'umd'
    }
    default:
      unimplemented(`output.format: ${format}`)
  }
}

function bindingifySourcemap(
  sourcemap: NormalizedOutputOptions['sourcemap'],
): BindingOutputOptions['sourcemap'] {
  switch (sourcemap) {
    case true:
      return 'file'
    case 'inline':
      return 'inline'
    case false:
    case undefined:
      return undefined
    case 'hidden':
      return 'hidden'
    default:
      throw new Error(`unknown sourcemap: ${sourcemap}`)
  }
}
