import { OutputOptions as RollupOutputOptions } from '../rollup-types'
import { OutputOptions as BindingOutputOptions } from '@rolldown/node-binding'
import { unimplemented } from '../utils'

export interface OutputOptions {
  dir?: RollupOutputOptions['dir']
  format?: 'esm'
  exports?: RollupOutputOptions['exports']
  sourcemap?: RollupOutputOptions['sourcemap']
}

function normalizeFormat(
  format: OutputOptions['format'],
): BindingOutputOptions['format'] {
  if (format == null || format === 'esm' || format === 'cjs') {
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

export function normalizeOutputOptions(
  opts: OutputOptions,
): BindingOutputOptions {
  const { dir, format, exports, sourcemap } = opts
  return {
    dir: dir,
    format: normalizeFormat(format),
    exports,
    sourcemap: normalizeSourcemap(sourcemap),
  }
}
