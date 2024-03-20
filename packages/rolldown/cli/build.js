import path from 'node:path'
import { performance } from 'node:perf_hooks'
import consola from 'consola'
import { colors } from 'consola/utils'
import { rolldown } from '../dist/index.mjs'

/**
 * @typedef {import('../src/rollup.d.ts').RollupOptions} RollupOptions
 */

/**
 * Build
 *
 * @param {RollupOptions | RollupOptions[]} options - Rollup options
 */
export default async function build(options) {
  const _options = Array.isArray(options) ? options : [options]

  for (const option of _options) {
    await buildWithRolldown(option)
  }
}

/**
 * Build with rolldown
 *
 * @param {RollupOptions} option - Rollup option
 */
async function buildWithRolldown(option) {
  const outputOptions = Array.isArray(option.output)
    ? option.output
    : [option.output]
  const files = outputOptions.map((output) =>
    relativeId(output.file || output.dir),
  )
  // TODO: multiple input files
  const inputFiles = option.input

  consola.info(`${colors.bold(inputFiles)} ...`)
  // consola.info(
  //   `${colors.bold(inputFiles)} → ${colors.bold(files.join(', '))}...`,
  // )

  const start = performance.now()
  const bundle = await rolldown(option)
  const ret = await Promise.all(
    outputOptions.map((output) => {
      consola.debug('output', output)
      return bundle.write(output)
    }),
  )
  ret.forEach((item, index) => {
    consola.debug(index, item)
  })
  // TODO: bundle.close is not a function ...
  // await bundle.close()

  // consola.success(
  //   `created ${colors.bold(files.join(', '))} in ${colors.bold((performance.now() - start).toFixed(2))}ms`,
  // )
  consola.success(
    `created ${colors.bold(files.join(', '))} in ${colors.bold((performance.now() - start).toFixed(2))} ms`,
  )
}

/**
 * Resolves a relative id to an absolute id.
 *
 * @param {string} id - An id to resolve
 * @returns {string} - A resolved relative id
 */
function relativeId(id) {
  return !path.isAbsolute(id) ? id : path.relative(path.resolve(), id)
}
