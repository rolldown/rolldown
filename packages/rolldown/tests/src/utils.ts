import { RollupOutput, RolldownOutputChunk } from '../../src'
import nodePath from 'node:path'
import nodeUrl from 'node:url'
import assert from 'node:assert'

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

/**
 *
 * @returns The absolute path to the `${WORKSPACE}/packages/rolldown/tests` directory
 */
export function testsDir(...joined: string[]) {
  const __dirname = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url))
  return nodePath.resolve(__dirname, '..', ...joined)
}

/**
 *
 * @returns The absolute path to the `${WORKSPACE}/packages/rolldown` directory
 */
export function projectDir(...joined: string[]) {
  return testsDir('..')
}

assert.deepEqual(testsDir().split(nodePath.sep).slice(-4), [
  'rolldown',
  'packages',
  'rolldown',
  'tests',
])
