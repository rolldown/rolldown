import path from 'node:path'
import * as rolldown from 'rolldown'
import * as esbuild from 'esbuild'
import * as rollup from 'rollup'
import { nodeResolve } from '@rollup/plugin-node-resolve'
import commonjs from '@rollup/plugin-commonjs'
import { PROJECT_ROOT } from './utils.js'

/**
 * @typedef {import('./types.js').BenchSuite} BenchSuite
 * @typedef {import('./types.js').RolldownBenchSuite} RolldownBenchSuite
 */

/**
 * @param {BenchSuite} suite
 * @returns {RolldownBenchSuite[]}
 */
export function getRolldownSuiteList(suite) {
  const rolldownOptionsList = Array.isArray(suite.rolldownOptions)
    ? suite.rolldownOptions
    : [{ name: 'default', options: suite.rolldownOptions }]
  return rolldownOptionsList.map(({ name, options }) => ({
    suiteName: name,
    title: suite.title,
    inputs: suite.inputs,
    options,
  }))
}

/**
 * @param {RolldownBenchSuite} suite
 */
export async function runRolldown(suite) {
  const { output: outputOptions = {}, ...inputOptions } = suite.options ?? {}
  const build = await rolldown.rolldown({
    platform: 'node',
    input: suite.inputs,
    ...inputOptions,
  })
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rolldown/${suite.title}`),
    ...outputOptions,
  })
  await build.close()
}

/**
 * @param {BenchSuite} suite
 */
export async function runEsbuild(suite) {
  const options = suite.esbuildOptions ?? {}
  await esbuild.build({
    platform: 'node',
    entryPoints: suite.inputs,
    bundle: true,
    outdir: path.join(PROJECT_ROOT, `./dist/esbuild/${suite.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    ...options,
  })
}

/**
 * @param {BenchSuite} suite
 */
export async function runRollup(suite) {
  const { output: outputOptions = {}, ...inputOptions } =
    suite.rollupOptions ?? {}
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
    ...inputOptions,
  })
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${suite.title}`),
    ...outputOptions,
  })
}
