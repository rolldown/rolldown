import { suites } from './src/suites.js'
import { createBenchmarks } from './src/utils.js'

createBenchmarks(suites, { sourcemap: false })
createBenchmarks(suites, { sourcemap: true })
