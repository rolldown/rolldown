import { bench } from 'vitest'
import { suitesForCI } from './src/suites.js'
import { runRolldown } from './src/run-bundler.js'
import { Bench } from 'tinybench'
import nodeAssert from 'node:assert'

async function collectRealBenchmarkData() {
  const bench = new Bench()

  for (const suite of suitesForCI) {
    bench.add(suite.title, async () => {
      await runRolldown(suite, false)
    })
    bench.add(`${suite.title}-sourcemap`, async () => {
      await runRolldown(suite, true)
    })
  }

  console.log('Warming up')
  await bench.warmup()
  console.log('Running benchmarks')
  await bench.run()
  console.table(bench.table())

  return Object.fromEntries(
    bench.tasks.map((task) => {
      if (!task.result) {
        throw new Error('Task has no result')
      }

      return [
        task.name,
        {
          hz: task.result.hz,
          mean: task.result.mean,
          p75: task.result.p75,
          p99: task.result.p99,
          p995: task.result.p995,
          p999: task.result.p999,
        },
      ]
    }),
  )
}

// Some contexts for why we need to collect real benchmark data rather just run benchmarks directly:
// - https://github.com/rolldown/rolldown/pull/699
// - https://github.com/CodSpeedHQ/action/issues/96
const realBenchData = await collectRealBenchmarkData()

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
  console.log('suite.title:', suite.title)
  bench(suite.title, async () => {
    sleep(realData.mean)
  })
  bench(`${suite.title}-sourcemap`, async () => {
    sleep(realDataSourceMap.mean)
  })
}

async function sleep(ms: number) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}
