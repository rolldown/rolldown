import path from 'node:path'
import * as rolldown from 'rolldown'
import * as esbuild from 'esbuild'
import * as rollup from 'rollup'
import { nodeResolve } from '@rollup/plugin-node-resolve'
import commonjs from '@rollup/plugin-commonjs'
import { PROJECT_ROOT } from './utils.js'

/**
 * @typedef {import('./types.js').BenchSuite} BenchSuite
 */

/**
 *
 * @param {BenchSuite} suite
 * @param {boolean} sourcemap
 */
export async function runRolldown(suite, sourcemap) {
  const build = await rolldown.rolldown({
    input: suite.inputs,
  })
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rolldown/${suite.title}`),
    sourcemap,
  })
}

/**
 * @param {BenchSuite} suite
 * @param {boolean} sourcemap
 */
export async function runEsbuild(suite, sourcemap) {
  await esbuild.build({
    entryPoints: suite.inputs,
    bundle: true,
    outdir: path.join(PROJECT_ROOT, `./dist/esbuild/${suite.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    sourcemap,
  })
}

/**
 * @param {BenchSuite} suite
 * @param {boolean} sourcemap
 */
export async function runRollup(suite, sourcemap) {
  const build = await rollup.rollup({
    input: suite.inputs,
    onwarn: (_warning, _defaultHandler) => {
      // ignore warnings
    },
    plugins: [
      nodeResolve({
        exportConditions: ['import'],
        mainFields: ['module', 'browser', 'main'],
      }),
      // @ts-expect-error
      commonjs(),
    ],
  })
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${suite.title}`),
    sourcemap,
  })
}
