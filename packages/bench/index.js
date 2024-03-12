// https://github.com/tinylibs/tinybench
import { Bench } from 'tinybench'
import path from 'node:path'
import url from 'node:url'
import fs from 'node:fs'
import * as rolldown from 'rolldown'
import * as esbuild from 'esbuild'
import * as rollup from 'rollup'
import { nodeResolve } from '@rollup/plugin-node-resolve'
import commonjs from '@rollup/plugin-commonjs'

const dirname = path.dirname(url.fileURLToPath(import.meta.url))
const repoRoot = path.join(dirname, '../../')

/**
 * @typedef BenchSuite
 * @property {string} title
 * If the `bundler` is not specified, it will run both in `esbuild` and `rolldown`
 * @property {'esbuild' | 'rolldown' | 'rollup'} [bundler]
 * @property {string[]} inputs
 * @property {number} [benchIteration]
 */

/**
 * @type {BenchSuite[]}
 */
const suites = [
  {
    title: 'threejs',
    inputs: [path.join(repoRoot, './temp/three/entry.js')],
  },
  {
    title: 'threejs10x',
    inputs: [path.join(repoRoot, './temp/three10x/entry.js')],
    benchIteration: 3,
  },
  {
    title: 'vue-stack',
    inputs: [path.join(dirname, 'vue-entry.js')],
  },
  {
    title: 'react-stack',
    inputs: ['react', 'react-dom'],
  },
]

/**
 * @param {BenchSuite} item
 */
async function runRolldown(item) {
  const build = await rolldown.rolldown({
    input: item.inputs,
    resolve: {
      // TODO
      // For now these are needed to align better w/ esbuild & Vite behavior
      // because internally we are still using the default behavior of oxc
      // resolver. We should ship a more sensible resolver default that aligns
      // with Vite's.
      conditionNames: ['import'],
      mainFields: ['module', 'browser', 'main'],
    },
  })
  await build.write({
    dir: path.join(dirname, `./dist/rolldown/${item.title}`),
  })
}

/**
 * @param {BenchSuite} item
 */
async function runEsbuild(item) {
  await esbuild.build({
    entryPoints: item.inputs,
    bundle: true,
    outdir: path.join(dirname, `./dist/esbuild/${item.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    sourcemap: false,
  })
}

/**
 * @param {BenchSuite} item
 */
async function runRollup(item) {
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
      // @ts-expect-error Something is wrong with the types
      commonjs(),
    ],
  })
  await build.write({
    dir: path.join(dirname, `./dist/rollup/${item.title}`),
  })
}

for (const suite of suites) {
  const bench = new Bench({ time: 100, iterations: suite.benchIteration ?? 10 })

  // Check if inputs have been initialized
  for (const input of suite.inputs) {
    if (input.includes('/') && !fs.existsSync(input)) {
      throw new Error(
        `Benchmark input ${input} not found, try running \`just setup-bench\` first.`,
      )
    }
  }

  if (!suite.bundler || suite.bundler === 'rolldown') {
    bench.add(`rolldown-${suite.title}`, async () => {
      try {
        await runRolldown(suite)
      } catch (err) {
        console.error(err)
      }
    })
  }
  if (!suite.bundler || suite.bundler === 'esbuild') {
    bench.add(`esbuild-${suite.title}`, async () => {
      try {
        await runEsbuild(suite)
      } catch (err) {
        console.error(err)
      }
    })
  }
  if (!suite.bundler || suite.bundler === 'rollup') {
    bench.add(`rollup-${suite.title}`, async () => {
      try {
        await runRollup(suite)
      } catch (err) {
        console.error(err)
      }
    })
  }

  await bench.run()

  const statusTable = bench.tasks.map(({ name: t, result: e }) => {
    if (!e) {
      console.error(`${t} failed:`, e)
      return null
    } else {
      const nsAverageTime = e.mean * 1e3 * 1e3
      const msAverageTime = nsAverageTime / 1e6
      return {
        'Task Name': t,
        'ops/sec': e.error
          ? 'NaN'
          : parseInt(e.hz.toString(), 10).toLocaleString(),
        'Average Time (ms)': e.error ? 'NaN' : msAverageTime.toFixed(2),
        Margin: e.error ? 'NaN' : `\xB1${e.rme.toFixed(2)}%`,
        Samples: e.error ? 'NaN' : e.samples.length,
      }
    }
  })
  console.table(statusTable)
}
