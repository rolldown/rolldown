import type { RolldownOptions } from '../types/rolldown-options'
import type { RolldownOutput } from '../types/rolldown-output'
import { rolldown } from './rolldown'

export interface BuildOptions extends RolldownOptions {
  /**
   * Write the output to the file system
   *
   * @default true
   */
  write?: boolean
}

async function build(options: BuildOptions): Promise<RolldownOutput>
/**
 * Build multiple outputs __sequentially__.
 */
async function build(options: BuildOptions[]): Promise<RolldownOutput[]>
async function build(
  options: BuildOptions | BuildOptions[],
): Promise<RolldownOutput | RolldownOutput[]> {
  if (Array.isArray(options)) {
    return Promise.all(options.map((opts) => build(opts)))
  } else {
    const { output, write = true, ...inputOptions } = options
    const build = await rolldown(inputOptions)
    if (write) {
      return build.write(output)
    } else {
      return build.generate(output)
    }
  }
}

export { build }
