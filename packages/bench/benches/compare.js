import { suites } from '../src/suites.js'
import * as bencher from '../src/bencher.js'
import { runEsbuild, runRolldown, runRollup } from '../src/run-bundler.js'

for (const suite of suites) {
  const group = bencher.group(suite.title, (bench) => {
    bench.add(`rolldown`, async () => {
      await runRolldown(suite, false)
    })
    bench.add(`esbuild`, async () => {
      await runEsbuild(suite, false)
    })
    if (suite.disableRollup) {
      return
    }
    bench.add(`rollup`, async () => {
      await runRollup(suite, false)
    })
  })
  const result = await group.run()
  // console.table(result.raw)
  result.display()
}

for (const suite of suites) {
  const group = bencher.group(`${suite.title}-sourcemap`, (bench) => {
    bench.add(`rolldown`, async () => {
      await runRolldown(suite, false)
    })
    bench.add(`esbuild`, async () => {
      await runEsbuild(suite, false)
    })
    if (suite.disableRollup) {
      return
    }
    bench.add(`rollup`, async () => {
      await runRollup(suite, false)
    })
  })
  const result = await group.run()
  result.display()
}
