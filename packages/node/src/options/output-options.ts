import { OutputOptions as RollupOutputOptions } from '../rollup-types'
import { OutputOptions as BindingOutputOptions } from '@rolldown/node-binding'
import { unimplemented } from '../utils'

export interface OutputOptions {
  dir?: RollupOutputOptions['dir']
  format?: 'esm'
  exports?: RollupOutputOptions['exports']
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

export function normalizeOutputOptions(
  opts: OutputOptions,
): BindingOutputOptions {
  const { dir, format, exports } = opts
  return {
    dir: dir,
    format: normalizeFormat(format),
    exports,
  }
}
