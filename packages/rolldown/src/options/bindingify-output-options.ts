import type { BindingOutputOptions } from '../binding'
import type { NormalizedOutputOptions } from './normalized-output-options'

export type InternalModuleFormat = 'es' | 'cjs'

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
    entryFileNames,
    chunkFileNames,
    assetFileNames,
    banner,
    footer,
  } = outputOptions
  return {
    dir,
    format: (function () {
      switch (format) {
        case 'es':
          return 'es'
        case 'cjs':
          return 'cjs'
      }
    })(),
    exports,
    sourcemap: bindingifySourcemap(sourcemap),
    sourcemapIgnoreList,
    sourcemapPathTransform,
    banner,
    footer,
    entryFileNames,
    chunkFileNames,
    assetFileNames,
    // TODO(sapphi-red): support parallel plugins
    plugins: [],
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
