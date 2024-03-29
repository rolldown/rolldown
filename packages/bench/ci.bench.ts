import { bench } from 'vitest'
import { suitesForCI } from './src/suites.js'
import { runRolldown } from './src/run-bundler.js'

// No `describe(...)` here, codspeed has different naming logic, so names we passed to `bench(...)` are for getting better
// readability in the codspeed dashboard not for vitest.
// Please refer to `compare.bench.ts` for better readability in the local.
for (const suite of suitesForCI) {
  bench(suite.title, async () => {
    await runRolldown(suite, false)
  })
  bench(`${suite.title}-sourcemap`, async () => {
    await runRolldown(suite, true)
  })
}
