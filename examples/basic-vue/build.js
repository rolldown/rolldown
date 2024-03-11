import { performance } from 'node:perf_hooks'
import * as rolldown from 'rolldown'

const start = performance.now()

const build = await rolldown.rolldown({
  input: './index.js',
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
})

await build.write()

console.log(`bundled in ${(performance.now() - start).toFixed(2)}ms`)
