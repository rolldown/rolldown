import nodeUtil from 'node:util'
import { suites } from '../src/suites.js'
import * as bencher from '../src/bencher.js'
import { runEsbuild, runRolldown, runRollup } from '../src/run-bundler.js'

console.log(
  nodeUtil.inspect(suites, { depth: null, colors: true, showHidden: false }),
)

for (const suite of suites) {
  const excludedBundlers = Array.isArray(suite.disableBundler)
    ? suite.disableBundler
    : suite.disableBundler
      ? [suite.disableBundler]
      : []

  const group = bencher.group(suite.title, (bench) => {
    if (!excludedBundlers.includes(`rolldown`)) {
      bench.add(`rolldown`, async () => {
        await runRolldown(suite)
      })
    }
    if (!excludedBundlers.includes(`esbuild`)) {
      bench.add(`esbuild`, async () => {
        await runEsbuild(suite)
      })
    }
    if (!excludedBundlers.includes(`rollup`)) {
      bench.add(`rollup`, async () => {
        await runRollup(suite)
      })
    }
  })
  const result = await group.run()
  result.display()
}
