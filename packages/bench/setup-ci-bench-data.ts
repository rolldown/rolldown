import { suitesForCI } from './src/suites.js'
import { runRolldown } from './src/run-bundler.js'
import { Bench } from 'tinybench'
import nodePath from 'node:path'
import fsExtra from 'fs-extra'
import { PROJECT_ROOT } from './src/constants.js'

async function setupBenchmarkDataForCI() {
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

  const data = Object.fromEntries(
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
          p999: task.result.p999,
        },
      ]
    }),
  )
  fsExtra.writeFileSync(
    nodePath.join(PROJECT_ROOT, 'dist/ci-bench-data.json'),
    JSON.stringify(data, null, 2),
  )
}

setupBenchmarkDataForCI()
