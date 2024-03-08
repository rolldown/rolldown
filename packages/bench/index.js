// https://github.com/tinylibs/tinybench
import { Bench } from 'tinybench'
import path from 'node:path'
import url from 'node:url'
import * as rolldown from '@rolldown/node'
import * as esbuild from 'esbuild'

const dirname = path.dirname(url.fileURLToPath(import.meta.url))
const repoRoot = path.join(dirname, '../../')

/**
 * @typedef BenchSuite
 * @property {string} title
 * If the `bundler` is not specified, it will run both in `esbuild` and `rolldown`
 * @property {'esbuild' | 'rolldown'} [bundler]
 * @property {string[]} inputs
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
  },
  {
    title: 'react_and_react_dom',
    inputs: ['react', 'react-dom'],
  },
]

/**
 * @param {BenchSuite} item
 */
async function runRolldown(item) {
  const build = await rolldown.rolldown({
    input: item.inputs,
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

for (const suite of suites) {
  const bench = new Bench({ time: 100 })

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

  await bench.run()

  console.table(bench.table())
}
