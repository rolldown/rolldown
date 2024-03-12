import { RollupOutput } from '../src'

export function getOutputChunkNames(output: RollupOutput) {
  return output.output
    .filter((chunk) => chunk.type === 'chunk')
    .map((chunk) => chunk.fileName)
    .sort()
}
