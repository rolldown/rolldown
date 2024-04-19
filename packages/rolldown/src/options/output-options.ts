import { OutputOptions as RollupOutputOptions } from '../rollup-types'
import { BindingOutputOptions } from '../binding'
import { noop, unimplemented } from '../utils'

export interface OutputOptions {
  dir?: RollupOutputOptions['dir']
  format?: 'es'
  exports?: RollupOutputOptions['exports']
  sourcemap?: RollupOutputOptions['sourcemap']
  sourcemapIgnoreList?: RollupOutputOptions['sourcemapIgnoreList']
  banner?: RollupOutputOptions['banner']
  footer?: RollupOutputOptions['footer']
  entryFileNames?: string
  chunkFileNames?: string
}

export type NormalizedOutputOptions = BindingOutputOptions

function normalizeFormat(
  format: OutputOptions['format'],
): BindingOutputOptions['format'] {
  if (format == null || format === 'es' || format === 'cjs') {
    return format
  } else {
    return unimplemented(`output.format: ${format}`)
  }
}

function normalizeSourcemap(
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

function normalizeSourcemapIgnoreList(
  sourcemapIgnoreList: OutputOptions['sourcemapIgnoreList'],
): BindingOutputOptions['sourcemapIgnoreList'] {
  return typeof sourcemapIgnoreList === 'function'
    ? sourcemapIgnoreList
    : sourcemapIgnoreList === false
      ? () => false
      : (relativeSourcePath: string, sourcemapPath: string) =>
          relativeSourcePath.includes('node_modules')
}

const getAddon = <T extends 'banner' | 'footer'>(
  config: OutputOptions,
  name: T,
): BindingOutputOptions[T] => {
  const configAddon = config[name]
  if (configAddon === undefined) return undefined
  if (typeof configAddon === 'function') {
    return configAddon as BindingOutputOptions[T]
  }
  return () => configAddon || ''
}

export function normalizeOutputOptions(
  opts: OutputOptions,
): BindingOutputOptions {
  const {
    dir,
    format,
    exports,
    sourcemap,
    sourcemapIgnoreList,
    entryFileNames,
    chunkFileNames,
  } = opts
  return {
    dir: dir,
    format: normalizeFormat(format),
    exports,
    sourcemap: normalizeSourcemap(sourcemap),
    sourcemapIgnoreList: normalizeSourcemapIgnoreList(sourcemapIgnoreList),
    // TODO(sapphi-red): support parallel plugins
    plugins: [],
    banner: getAddon(opts, 'banner'),
    footer: getAddon(opts, 'footer'),
    entryFileNames,
    chunkFileNames,
  }
}
