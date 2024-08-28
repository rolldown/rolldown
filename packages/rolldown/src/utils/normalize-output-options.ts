import type { OutputOptions } from '../options/output-options'
import { unimplemented } from './misc'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'

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
    globals,
    entryFileNames,
    chunkFileNames,
    assetFileNames,
    name,
    esModule,
  } = opts
  return {
    dir: dir,
    format: getFormat(format),
    exports: exports ?? 'auto',
    sourcemap: sourcemap ?? false,
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
    esModule: esModule ?? 'if-default-prop',
    // TODO support functions
    globals: globals ?? {},
    entryFileNames: entryFileNames ?? '[name].js',
    chunkFileNames: chunkFileNames ?? '[name]-[hash].js',
    assetFileNames: assetFileNames ?? 'assets/[name]-[hash][extname]',
    plugins: [],
    minify: opts.minify,
    extend: opts.extend,
    name,
    externalLiveBindings: opts.externalLiveBindings ?? true,
    inlineDynamicImports: opts.inlineDynamicImports ?? false,
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

    case 'iife': {
      return 'iife'
    }

    default:
      unimplemented(`output.format: ${format}`)
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
