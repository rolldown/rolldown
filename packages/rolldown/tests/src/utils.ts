import {
  RollupOutput,
  RolldownOutputChunk,
  RolldownOutputAsset,
} from '../../src'
import nodePath from 'node:path'
import assert from 'node:assert'
import { workspaceRoot } from '@rolldown/testing'

export function getOutputChunkNames(output: RollupOutput) {
  return output.output
    .filter((chunk) => chunk.type === 'chunk')
    .map((chunk) => chunk.fileName)
}

export function getOutputChunk(output: RollupOutput): RolldownOutputChunk[] {
  return output.output.filter(
    (chunk) => chunk.type === 'chunk',
  ) as RolldownOutputChunk[]
}

export function getOutputAsset(output: RollupOutput): RolldownOutputAsset[] {
  return output.output.filter(
    (chunk) => chunk.type === 'asset',
  ) as RolldownOutputAsset[]
}

export function getOutputFileNames(output: RollupOutput) {
  return output.output.map((chunk) => chunk.fileName).sort()
}

export function getOutputAssetNames(output: RollupOutput) {
  return output.output
    .filter((chunk) => chunk.type === 'asset')
    .map((chunk) => chunk.fileName)
    .sort()
}

/**
 *
 * @returns The absolute path to the `${WORKSPACE}/packages/rolldown` directory
 */
export function projectDir(...joined: string[]) {
  return workspaceRoot('packages/rolldown', ...joined)
}

/**
 *
 * @returns The absolute path to the `${WORKSPACE}/packages/rolldown/tests` directory
 */
export function testsDir(...joined: string[]) {
  return projectDir('tests', ...joined)
}

assert.deepEqual(testsDir().split(nodePath.sep).slice(-4), [
  'rolldown',
  'packages',
  'rolldown',
  'tests',
])

export function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
