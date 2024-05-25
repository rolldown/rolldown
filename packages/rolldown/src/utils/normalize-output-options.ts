import type { OutputOptions } from '@src/options/output-options'
import { unimplemented } from '.'
import type { NormalizedOutputOptions } from '@src/options/normalized-output-options'

export function normalizeOutputOptions(
  opts: OutputOptions,
): NormalizedOutputOptions {
  const {
    dir,
    format,
    exports,
    sourcemap,
    sourcemapIgnoreList,
    sourcemapPathTransform,
    entryFileNames,
    chunkFileNames,
  } = opts
  return {
    dir: dir,
    format: getFormat(format),
    exports: exports ?? 'named',
    sourcemap: sourcemap ?? false,
    sourcemapIgnoreList:
      typeof sourcemapIgnoreList === 'function'
        ? sourcemapIgnoreList
        : sourcemapIgnoreList === false
          ? () => false
          : (relativeSourcePath: string, sourcemapPath: string) =>
              relativeSourcePath.includes('node_modules'),
    sourcemapPathTransform,
    banner: getAddon(opts, 'banner'),
    footer: getAddon(opts, 'footer'),
    entryFileNames: entryFileNames ?? '[name].js',
    chunkFileNames: chunkFileNames ?? '[name]-[hash].js',
    plugins: [],
  }
}

function getFormat(
  format: OutputOptions['format'],
): NormalizedOutputOptions['format'] {
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

    default:
      unimplemented(`output.format: ${format}`)
  }
}

const getAddon = <T extends 'banner' | 'footer'>(
  config: OutputOptions,
  name: T,
): NormalizedOutputOptions[T] => {
  const configAddon = config[name]
  if (typeof configAddon === 'function') {
    return configAddon as NormalizedOutputOptions[T]
  }
  // TODO Here should be remove async
  return async () => configAddon || ''
}
