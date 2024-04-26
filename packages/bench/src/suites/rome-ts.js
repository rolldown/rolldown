import { defineSuite } from '../utils.js'
import nodePath from 'node:path'
import * as esbuild from 'esbuild'
import { REPO_ROOT } from '../utils.js'
import _ from 'lodash'
import {
  default as parallelBabelPluginAsync,
  syncVersion as parallelBabelPluginSync,
} from '../parallel-babel-plugin/index.js'
import { babelPlugin } from '../parallel-babel-plugin/impl.js'
import { builtinModules } from 'node:module'

const inputs = [nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts')]

const esbuildOptions = {
  tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
}

/**
 * @type {import('../types.js').BenchSuite['rolldownOptions']}
 */
const rolldownOptions = [
  {
    name: 'esbuild',
    options: {
      logLevel: 'silent',
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
      warmupFiles: [
        nodePath.join(REPO_ROOT, './tmp/bench/rome/src/rome/**/*.ts'),
        // nodePath.join(REPO_ROOT, './tmp/bench/rome/src/@romejs/**/*.ts'),
      ],
      warmupFilesExclude: ['**/test-fixtures/**/*.ts', '**/*.test.ts'],
    },
  },
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
      warmupFiles: [
        nodePath.join(REPO_ROOT, './tmp/bench/rome/src/rome/**/*.ts'),
        // nodePath.join(REPO_ROOT, './tmp/bench/rome/src/@romejs/**/*.ts'),
      ],
      warmupFilesExclude: ['**/test-fixtures/**/*.ts'],
    },
  },
  {
    name: 'js-parallel-babel-sync',
    options: {
      logLevel: 'silent',
      external: builtinModules,
      // Need this due rome is not written with `isolatedModules: true`
      shimMissingExports: true,
      plugins: [parallelBabelPluginAsync()],
      resolve: {
        extensions: ['.ts'],
        tsconfigFilename: nodePath.join(
          REPO_ROOT,
          './tmp/bench/rome/src/tsconfig.json',
        ),
      },
      warmupFiles: [
        nodePath.join(REPO_ROOT, './tmp/bench/rome/src/rome/**/*.ts'),
        // nodePath.join(REPO_ROOT, './tmp/bench/rome/src/@romejs/**/*.ts'),
      ],
      warmupFilesExclude: ['**/test-fixtures/**/*.ts'],
    },
  },
  {
    name: 'js-parallel-babel-async',
    options: {
      logLevel: 'silent',
      external: builtinModules,
      // Need this due rome is not written with `isolatedModules: true`
      shimMissingExports: true,
      plugins: [parallelBabelPluginSync()],
      resolve: {
        extensions: ['.ts'],
        tsconfigFilename: nodePath.join(
          REPO_ROOT,
          './tmp/bench/rome/src/tsconfig.json',
        ),
      },
      warmupFiles: [
        nodePath.join(REPO_ROOT, './tmp/bench/rome/src/rome/**/*.ts'),
        // nodePath.join(REPO_ROOT, './tmp/bench/rome/src/@romejs/**/*.ts'),
      ],
      warmupFilesExclude: ['**/test-fixtures/**/*.ts'],
    },
  },
]

export const suiteRomeTsWithBabelAndParallelism = defineSuite({
  title: 'rome-ts-babel-parallelism',
  disableBundler: ['rollup', 'esbuild'],
  inputs,
  rolldownOptions,
})

export const suiteRomeTs = defineSuite({
  title: 'rome-ts',
  inputs,
  esbuildOptions,
  rolldownOptions,
  disableBundler: ['rollup'],
})
