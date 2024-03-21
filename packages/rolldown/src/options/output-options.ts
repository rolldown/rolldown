import { OutputOptions as RollupOutputOptions } from '../rollup-types'
import { BindingOutputOptions } from '../binding'
import { unimplemented } from '../utils'

export interface OutputOptions {
  dir?: RollupOutputOptions['dir']
  format?: 'es'
  exports?: RollupOutputOptions['exports']
  sourcemap?: RollupOutputOptions['sourcemap']
  banner?: string
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

export function normalizeOutputOptions(
  opts: OutputOptions,
): BindingOutputOptions {
  const { dir, format, exports, sourcemap, banner } = opts
  return {
    dir: dir,
    format: normalizeFormat(format),
    exports,
    sourcemap: normalizeSourcemap(sourcemap),
    plugins: [],
    banner,
  }
}
