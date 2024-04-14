import { defineSuite } from '../utils.js'
import nodePath from 'node:path'
import * as esbuild from 'esbuild'
import { REPO_ROOT } from '../utils.js'
import _ from 'lodash'
import parallelBabelPlugin from '../parallel-babel-plugin/index.js'
import { babelPlugin } from '../parallel-babel-plugin/impl.js'
import { builtinModules } from 'node:module'

export const suiteRomeTs = defineSuite({
  title: 'rome-ts',
  inputs: [nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts')],
  esbuildOptions: {
    tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
  },
  rolldownOptions: [
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
      },
    },
    {
      name: 'js-parallel',
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
  ],
  disableBundler: ['rollup'],
})
