import * as tinyBench from 'tinybench'
import { suitesForCI } from '../src/suites.js'
import { runRolldown } from '../src/run-bundler.js'

const bench = new tinyBench.Bench()

for (const suite of suitesForCI) {
  bench.add(suite.title, async () => {
    await runRolldown(suite, false)
  })
}

for (const suite of suitesForCI) {
  bench.add(`${suite.title}-sourcemap`, async () => {
    await runRolldown(suite, false)
  })
}

await bench.warmup()
await bench.run()

const dataForGitHubBenchmarkAction = bench.tasks.map((task) => {
  if (!task.result) {
    throw new Error('Task result is empty for ' + task.name)
  }

  return {
    name: task.name,
    value: task.result.mean.toFixed(2),
    unit: 'ms',
  }
})

console.log(JSON.stringify(dataForGitHubBenchmarkAction, null, 2))
