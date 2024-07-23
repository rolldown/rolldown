import nodePath from 'node:path'
import { PROJECT_ROOT, REPO_ROOT } from '../utils.js'
import _ from 'lodash'
import { suiteRomeTs } from './rome-ts.js'

/**
 * @type {import('../types.js').BenchSuite[]}
 */
export const suitesForCI = [
  {
    title: 'threejs10x',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three10x/entry.js')],
    disableBundler: 'rollup',
    rolldownOptions: {
      logLevel: 'silent',
    },
    derived: {
      sourcemap: true,
      minify: true,
    },
  },
  suiteRomeTs,
]

/**
 * @type {import('../types.js').BenchSuite[]}
 */
export const suites = [
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three/entry.js')],
    rolldownOptions: {
      logLevel: 'silent',
    },
  },
  {
    title: 'vue-stack',
    inputs: [nodePath.join(PROJECT_ROOT, 'vue-entry.js')],
    derived: {
      sourcemap: true,
    },
  },
  {
    title: 'react-stack',
    inputs: ['react', 'react-dom'],
  },
  ...suitesForCI,
]

/**
 *
 * @param {import('../run-bundler.js').BenchSuite[]} suites
 */
export function expandSuitesWithDerived(suites) {
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
    if (suite.derived?.minify) {
      const derived = _.cloneDeepWith(suite, (value) => {
        // We should pay attention to this while using singletons in config
        if (typeof value === 'function') {
          return value
        }
      })
      derived.title = `${suite.title}-minify`
      delete derived.derived
      _.set(derived, 'esbuildOptions.minify', true)
      _.set(derived, 'rolldownOptions.output.minify', true)
      expanded.push(derived)
    }
    if (suite.derived?.minify && suite.derived?.sourcemap) {
      const derived = _.cloneDeepWith(suite, (value) => {
        // We should pay attention to this while using singletons in config
        if (typeof value === 'function') {
          return value
        }
      })
      derived.title = `${suite.title}-minify-sourcemap`
      delete derived.derived
      _.set(derived, 'esbuildOptions.sourcemap', true)
      _.set(derived, 'rolldownOptions.output.sourcemap', true)
      _.set(derived, 'esbuildOptions.minify', true)
      _.set(derived, 'rolldownOptions.output.minify', true)
      expanded.push(derived)
    }
    return expanded
  })
}
