import type { BindingOutputOptions } from '../binding'
import type { NormalizedOutputOptions } from './normalized-output-options'

export function bindingifyOutputOptions(
  outputOptions: NormalizedOutputOptions,
): BindingOutputOptions {
  const {
    dir,
    format,
    exports,
    sourcemap,
    sourcemapIgnoreList,
    sourcemapPathTransform,
    name,
    entryFileNames,
    chunkFileNames,
    assetFileNames,
    banner,
    footer,
    intro,
    outro,
  } = outputOptions
  return {
    dir,
    format: (function () {
      switch (format) {
        case 'es':
          return 'es'
        case 'cjs':
          return 'cjs'
        case 'iife':
          return 'iife'
      }
    })(),
    exports,
    sourcemap: bindingifySourcemap(sourcemap),
    sourcemapIgnoreList,
    sourcemapPathTransform,
    banner,
    footer,
    intro,
    outro,
    name,
    entryFileNames,
    chunkFileNames,
    assetFileNames,
    // TODO(sapphi-red): support parallel plugins
    plugins: [],
    minify: outputOptions.minify,
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
    case 'hidden':
      return 'hidden'

    default:
      throw new Error(`unknown sourcemap: ${sourcemap}`)
  }
}
