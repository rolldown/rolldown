import nodePath from 'node:path'
import * as esbuild from 'esbuild'
import { PROJECT_ROOT, REPO_ROOT } from './utils.js'
import _ from 'lodash'
import parallelBabelPlugin from './parallel-babel-plugin/index.js'
import { babelPlugin } from './parallel-babel-plugin/impl.js'
import { builtinModules } from 'node:module'

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
    rolldownOptions: [
      {
        name: 'esbuild',
        options: {
          external: builtinModules,
          // Need this due rome is not written with `isolatedModules: true`
          shimMissingExports: true,
          plugins: [
            {
              name: '@rolldown/plugin-esbuild',
              async transform(code, id) {
                const ext = nodePath.extname(id)
                if (ext === '.ts' || ext === '.tsx') {
                  const ret = await esbuild.transform(code, {
                    platform: 'node',
                    loader: ext === '.tsx' ? 'tsx' : 'ts',
                    format: 'esm',
                    target: 'chrome80',
                    sourcemap: true,
                  })

                  return {
                    code: ret.code,
                  }
                }
              },
            },
          ],
          resolve: {
            extensions: ['.ts'],
            tsconfigFilename: nodePath.join(
              REPO_ROOT,
              './tmp/bench/rome/src/tsconfig.json',
            ),
          },
        },
      },
      {
        name: 'js-single',
        options: {
          external: builtinModules,
          // Need this due rome is not written with `isolatedModules: true`
          shimMissingExports: true,
          plugins: [babelPlugin()],
          resolve: {
            extensions: ['.ts'],
            tsconfigFilename: nodePath.join(
              REPO_ROOT,
              './tmp/bench/rome/src/tsconfig.json',
            ),
          },
        },
      },
      {
        name: 'js-parallel',
        options: {
          external: builtinModules,
          // Need this due rome is not written with `isolatedModules: true`
          shimMissingExports: true,
          plugins: [parallelBabelPlugin()],
          resolve: {
            extensions: ['.ts'],
            tsconfigFilename: nodePath.join(
              REPO_ROOT,
              './tmp/bench/rome/src/tsconfig.json',
            ),
          },
        },
      },
    ],
    disableBundler: ['rollup'],
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
