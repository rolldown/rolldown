import { OutputOptions as RollupOutputOptions } from '../rollup-types'
import { BindingOutputOptions } from '../binding'
import { unimplemented } from '../utils'

export interface OutputOptions {
  dir?: RollupOutputOptions['dir']
  format?: 'es'
  exports?: RollupOutputOptions['exports']
  sourcemap?: RollupOutputOptions['sourcemap']
  banner?: RollupOutputOptions['banner']
  footer?: RollupOutputOptions['footer']
  entryFileNames?: string
  chunkFileNames?: string
}

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
  const { dir, format, exports, sourcemap, entryFileNames, chunkFileNames } =
    opts
  return {
    dir: dir,
    format: normalizeFormat(format),
    exports,
    sourcemap: normalizeSourcemap(sourcemap),
    plugins: [],
    banner: getAddon(opts, 'banner'),
    footer: getAddon(opts, 'footer'),
    entryFileNames,
    chunkFileNames,
  }
}
