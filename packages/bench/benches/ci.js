import * as tinyBench from 'tinybench'
import nodePath from 'path'
import nodeUrl from 'url'
import nodeFs from 'fs'
import { suitesForCI } from '../src/suites.js'
import { getRolldownSuiteList, runRolldown } from '../src/run-bundler.js'

const DIRNAME = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url))
const PROJECT_ROOT = nodePath.resolve(DIRNAME, '..')
const REPO_ROOT = nodePath.resolve(PROJECT_ROOT, '../..')

const bench = new tinyBench.Bench()

for (const suite of suitesForCI) {
  const rolldownSuiteList = getRolldownSuiteList(suite)
  for (const rolldownSuite of rolldownSuiteList) {
    bench.add(`rolldown (${rolldownSuite.suiteName})`, async () => {
      await runRolldown(rolldownSuite)
    })
  }
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
    unit: 'ms / ops',
  }
})

const serialized = JSON.stringify(dataForGitHubBenchmarkAction, null, 2)

console.log(serialized)

nodeFs.writeFileSync(
  nodePath.resolve(REPO_ROOT, 'tmp/new-benchmark-node-output.json'),
  serialized,
  'utf8',
)
