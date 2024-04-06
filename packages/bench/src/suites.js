import nodePath from 'node:path'
import { PROJECT_ROOT, REPO_ROOT } from './utils.js'
import _ from 'lodash'

/**
 * @type {import('./types.js').BenchSuite[]}
 */
export const rawSuitesForCI = [
  {
    title: 'threejs10x',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three10x/entry.js')],
    disableBundler: 'rollup',
    derived: {
      sourcemap: true,
    },
  },
]

export const suitesForCI = expandSuitesWithDerived(rawSuitesForCI)

/**
 * @type {import('./types.js').BenchSuite[]}
 */
export const suites = expandSuitesWithDerived([
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three/entry.js')],
  },
  {
    title: 'vue-stack',
    inputs: [nodePath.join(PROJECT_ROOT, 'vue-entry.js')],
    derived: {
      sourcemap: true,
    },
  },
  {
    title: 'rome-ts',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts')],
    esbuildOptions: {
      tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
    },
    rolldownOptions: {
      resolve: {
        extensions: ['.ts'],
      },
    },
    disableBundler: ['rolldown', 'rollup'],
  },
  {
    title: 'react-stack',
    inputs: ['react', 'react-dom'],
  },
  ...rawSuitesForCI,
])

/**
 *
 * @param {import('./run-bundler.js').BenchSuite[]} suites
 */
function expandSuitesWithDerived(suites) {
  return suites.flatMap((suite) => {
    const expanded = [suite]
    if (suite.derived?.sourcemap) {
      const derived = _.cloneDeepWith(suite, (value) => {
        // We should pay attention to this while using singletons in config
        if (typeof value === 'function') {
          return value
        }
      })
      derived.title = `${suite.title}-sourcemap`
      delete derived.derived
      _.set(derived, 'esbuildOptions.sourcemap', true)
      _.set(derived, 'rolldownOptions.output.sourcemap', true)
      _.set(derived, 'rollupOptions.output.sourcemap', true)
      expanded.push(derived)
    }
    return expanded
  })
}
