import fs from 'node:fs';
import path from 'node:path'
import assert from 'node:assert'

import type {
  RolldownOutput as RollupOutput,
  OutputChunk as RolldownOutputChunk,
  OutputAsset as RolldownOutputAsset,
} from 'rolldown'

/**
 * @description
 * - Get the absolute path to the root of the workspace. The root is always the directory containing the root `Cargo.toml`, `package.json`, `pnpm-workspace.yaml` etc.
 * - `workspaceRoot('packages')` equals to `path.resolve(workspaceRoot(), 'packages')`
 */
export function workspaceRoot(...joined: string[]) {
  return path.resolve(import.meta.dirname, '../../../..', ...joined);
}

assert(
  fs.existsSync(workspaceRoot('pnpm-workspace.yaml')),
  `${workspaceRoot('pnpm-workspace.yaml')} does not exist`,
);
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

assert.deepEqual(testsDir().split(path.sep).slice(-4), [
  'rolldown',
  'packages',
  'rolldown',
  'tests',
])

export function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

export function getLocation(source: string, search: string | number) {
  var lines = source.split('\n')
  var length_ = lines.length

  var lineStart = 0
  var index

  const charIndex = typeof search === 'number' ? search : source.indexOf(search)

  for (index = 0; index < length_; index += 1) {
    var line = lines[index]
    var lineEnd = lineStart + line.length + 1 // +1 for newline

    if (lineEnd > charIndex) {
      return { line: index + 1, column: charIndex - lineStart }
    }

    lineStart = lineEnd
  }

  throw new Error('Could not determine location of character')
}

export async function waitUtil(expectFn: () => void) {
  for (let tries = 0; tries < 20; tries++) {
    try {
      expectFn()
      return
    } catch {}
    await sleep(50)
  }
  expectFn()
}
