import { bench } from 'vitest'
import { suitesForCI } from './src/suites.js'
import nodeFs from 'node:fs'
import nodeAssert from 'node:assert'
import nodePath from 'node:path'
import { PROJECT_ROOT } from './src/constants.js'

function sleep(ms: number) {
  const now = performance.now()
  while (performance.now() - now < ms) {
    // Do nothing
  }
}

function main() {
  // Some contexts for why we need to collect real benchmark data rather just run benchmarks directly:
  // - https://github.com/rolldown/rolldown/pull/699
  // - https://github.com/CodSpeedHQ/action/issues/96
  const realBenchData = JSON.parse(
    nodeFs.readFileSync(
      nodePath.join(PROJECT_ROOT, 'dist/ci-bench-data.json'),
      'utf8',
    ),
  )

  console.log('realBenchData:')
  console.table(realBenchData)

  // No `vitest.describe(...)` here, codspeed has different naming logic, so names we passed to `bench(...)` are for getting better
  // readability in the codspeed dashboard not for vitest.
  // Please refer to `compare.bench.ts` for better readability in the local.
  for (const suite of suitesForCI) {
    const realData = realBenchData[suite.title]
    const realDataSourceMap = realBenchData[`${suite.title}-sourcemap`]
    nodeAssert(realData != null)
    nodeAssert(realDataSourceMap != null)
    bench(suite.title, () => {
      sleep(realData.mean)
    })
    bench(`${suite.title}-sourcemap`, () => {
      sleep(realDataSourceMap.mean)
    })
  }
}

main()
