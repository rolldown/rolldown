import { suitesForCI } from './src/suites.js'
import { createBenchmarks } from './src/utils.js'

createBenchmarks(suitesForCI, { sourcemap: false, rolldownOnly: true })
createBenchmarks(suitesForCI, { sourcemap: true, rolldownOnly: true })
