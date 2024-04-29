import type { RollupOutput, RolldownOutputChunk } from '../../src'
import type { TestConfig } from './types'

export function getOutputChunkNames(output: RollupOutput) {
  return output.output
    .filter((chunk) => chunk.type === 'chunk')
    .map((chunk) => chunk.fileName)
    .sort()
}

export function getOutputChunk(output: RollupOutput): RolldownOutputChunk[] {
  return output.output.filter(
    (chunk) => chunk.type === 'chunk',
  ) as RolldownOutputChunk[]
}

export function getOutputFileNames(output: RollupOutput) {
  return output.output.map((chunk) => chunk.fileName).sort()
}

export async function loadTestConfig(path: string): Promise<TestConfig> {
  return await import(path).then((m) => m.default)
}
