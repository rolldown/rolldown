import type { RolldownOptions } from '../types/rolldown-options'
import type { RolldownOutput } from '../types/rolldown-output'
import { rolldown } from '../rolldown'

export async function build(options: RolldownOptions): Promise<RolldownOutput> {
  const { output, ...inputOptions } = options
  const build = await rolldown(inputOptions)
  return build.generate(output)
}
