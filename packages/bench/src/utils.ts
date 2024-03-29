import nodePath from 'node:path'
import nodeUrl from 'node:url'
import { bench, describe } from 'vitest'
import { runRolldown, runEsbuild, runRollup } from './/run-bundler.js'
import { BenchSuite } from './suites.js'

const dirname = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url))

export const REPO_ROOT = nodePath.join(dirname, '../../..')

export const PROJECT_ROOT = nodePath.join(dirname, '..')

export function createBench(suite: BenchSuite, benchConfig: BenchConfig) {
  const suiteTitle = `${suite.title}${benchConfig.sourcemap ? '-sourcemap' : ''}`
  const enableSourcemap = benchConfig.sourcemap ?? false
  describe(suiteTitle, () => {
    bench(`rolldown`, async () => {
      await runRolldown(suite, enableSourcemap)
    })
    if (benchConfig.rolldownOnly) {
      return
    }
    bench(`esbuild`, async () => {
      await runEsbuild(suite, enableSourcemap)
    })
    bench(
      `rollup`,
      async () => {
        await runRollup(suite, enableSourcemap)
      },
      { iterations: 2 },
    )
  })
}

interface BenchConfig {
  sourcemap?: boolean
  rolldownOnly?: boolean
}

export function createBenchmarks(
  suites: BenchSuite[],
  benchConfig: BenchConfig = {},
) {
  for (const suite of suites) {
    createBench(suite, benchConfig)
  }
}
