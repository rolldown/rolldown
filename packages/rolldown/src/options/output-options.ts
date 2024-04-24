import { BindingOutputOptions, RenderedChunk } from '../binding'
import { unimplemented } from '../utils'

export type SourcemapIgnoreListOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => boolean

export type SourcemapPathTransformOption = (
  relativeSourcePath: string,
  sourcemapPath: string,
) => string

type AddonFunction = (chunk: RenderedChunk) => string | Promise<string>

export type InternalModuleFormat = 'es' | 'cjs'

export type ModuleFormat = InternalModuleFormat | 'esm' | 'module' | 'commonjs'

// Make sure port `OutputOptions` and `NormalizedOutputOptions` from rollup.
export interface OutputOptions {
  dir?: string
  format?: ModuleFormat
  exports?: 'default' | 'named' | 'none' | 'auto'
  sourcemap?: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList?: boolean | SourcemapIgnoreListOption
  sourcemapPathTransform?: SourcemapPathTransformOption
  banner?: string | AddonFunction
  footer?: string | AddonFunction
  entryFileNames?: string
  chunkFileNames?: string
}

export type NormalizedOutputOptions = {
  dir: string | undefined
  format: InternalModuleFormat
  exports: 'default' | 'named' | 'none' | 'auto'
  sourcemap: boolean | 'inline' | 'hidden'
  sourcemapIgnoreList: SourcemapIgnoreListOption
  sourcemapPathTransform: SourcemapPathTransformOption | undefined
  banner: AddonFunction
  footer: AddonFunction
  entryFileNames: string
  chunkFileNames: string
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
  return () => configAddon || ''
}

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
    exports: exports ?? 'auto',
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
  }
}

function getBindingSourcemap(
  sourcemap: OutputOptions['sourcemap'],
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

export function createOutputOptionsAdapter(
  opts: OutputOptions,
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
  } = outputOptions
  return {
    dir,
    format,
    exports,
    sourcemap: getBindingSourcemap(sourcemap),
    sourcemapIgnoreList,
    sourcemapPathTransform,
    // Note: Here using `NormalizedOutputOptions#banner` will caused an error at sourcemaps excludes-plugin-helpers test.
    banner: opts.banner && getAddon(opts, 'banner'),
    footer: opts.footer && getAddon(opts, 'footer'),
    entryFileNames,
    chunkFileNames,
    // TODO(sapphi-red): support parallel plugins
    plugins: [],
  }
}
