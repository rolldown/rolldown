import { BenchSuite } from './suites.js'
import path from 'node:path'
import * as rolldown from 'rolldown'
import * as esbuild from 'esbuild'
import * as rollup from 'rollup'
import { nodeResolve } from '@rollup/plugin-node-resolve'
import commonjs from '@rollup/plugin-commonjs'
import { PROJECT_ROOT } from './utils.js'

export async function runRolldown(suite: BenchSuite, sourcemap: boolean) {
  const build = await rolldown.rolldown({
    input: suite.inputs,
  })
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rolldown/${suite.title}`),
    sourcemap,
  })
}

/**
 * @param {BenchSuite} item
 * @param {boolean} sourcemap
 */
export async function runEsbuild(item: BenchSuite, sourcemap: boolean) {
  await esbuild.build({
    entryPoints: item.inputs,
    bundle: true,
    outdir: path.join(PROJECT_ROOT, `./dist/esbuild/${item.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    sourcemap,
  })
}

/**
 * @param {BenchSuite} item
 * @param {boolean} sourcemap
 */
export async function runRollup(item: BenchSuite, sourcemap: boolean) {
  const build = await rollup.rollup({
    input: item.inputs,
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
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${item.title}`),
    sourcemap,
  })
}
