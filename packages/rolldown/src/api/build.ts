import type { RolldownOptions } from '../types/rolldown-options'
import type { RolldownOutput } from '../types/rolldown-output'
import { rolldown } from './rolldown'

export interface BuildOptions extends RolldownOptions {
  /**
   * Write the output to the file system
   */
  write?: boolean
}

export async function build(options: BuildOptions): Promise<RolldownOutput> {
  const { output, ...inputOptions } = options
  const build = await rolldown(inputOptions)
  if (options.write) {
    return build.write(output)
  } else {
    return build.generate(output)
  }
}
