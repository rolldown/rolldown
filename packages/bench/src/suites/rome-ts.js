import { defineSuite } from '../utils.js'
import nodePath from 'node:path'
import { REPO_ROOT } from '../utils.js'
import _ from 'lodash'
import { default as parallelBabelPlugin } from '../parallel-babel-plugin/index.js'
import { babelPlugin } from '../parallel-babel-plugin/impl.js'
import { builtinModules } from 'node:module'

const inputs = [nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts')]

const esbuildOptions = {
  tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
}

/**
 * @type {import('../types.js').BenchSuite['rolldownOptions']}
 */
const rolldownOptionsForParallelism = [
  {
    name: 'js-single',
    options: {
      logLevel: 'silent',
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
    name: 'js-parallel-babel',
    options: {
      logLevel: 'silent',
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
]

export const suiteRomeTsWithBabelAndParallelism = defineSuite({
  title: 'rome-ts-babel-parallelism',
  disableBundler: ['rollup', 'esbuild'],
  inputs,
  rolldownOptions: rolldownOptionsForParallelism,
})

export const suiteRomeTs = defineSuite({
  title: 'rome-ts',
  inputs,
  esbuildOptions,
  rolldownOptions: {
    logLevel: 'silent',
    external: builtinModules,
    // Need this due rome is not written with `isolatedModules: true`
    shimMissingExports: true,
    resolve: {
      tsconfigFilename: nodePath.join(
        REPO_ROOT,
        './tmp/bench/rome/src/tsconfig.json',
      ),
    },
  },
  disableBundler: ['rollup'],
  derived: {
    sourcemap: true,
  },
})
