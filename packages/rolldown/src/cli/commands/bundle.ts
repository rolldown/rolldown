import { performance } from 'node:perf_hooks'
import { rolldown, watch as rolldownWatch } from '../../rolldown'
import type { RolldownOptions, RolldownOutput, RollupOutput } from '../../index'
import { arraify } from '../../utils/misc'
import { ensureConfig, logger } from '../utils'
import * as colors from '../colors'
import { NormalizedCliOptions } from '../arguments/normalize'

export async function bundleWithConfig(
  configPath: string,
  cliOptions: NormalizedCliOptions,
) {
  const config = await ensureConfig(configPath)

  if (!config) {
    logger.error(`No configuration found at ${config}`)
    process.exit(1)
  }

  const configList = arraify(config)

  for (const config of configList) {
    cliOptions.watch
      ? await watchInner(config, cliOptions)
      : bundleInner(config, cliOptions)
  }
}

export async function bundleWithCliOptions(cliOptions: NormalizedCliOptions) {
  // TODO when supports `output.file`, we should modify it here.
  if (cliOptions.output.dir) {
    cliOptions.watch
      ? await watchInner({}, cliOptions)
      : await bundleInner({}, cliOptions)
  } else if (!cliOptions.watch) {
    const build = await rolldown(cliOptions.input)
    const { output } = await build.generate(cliOptions.output)
    if (output.length > 1) {
      logger.error('Multiple chunks are not supported to display in stdout')
      process.exit(1)
    } else if (output.length === 0) {
      logger.error('No output generated')
      process.exit(1)
    } else {
      logger.log(output[0].code)
    }
  } else {
    logger.error('You must specify `output.dir` to use watch mode')
    process.exit(1)
  }
}

async function watchInner(
  options: RolldownOptions,
  cliOptions: NormalizedCliOptions,
) {
  // Only if watch is true in CLI can we use watch mode.
  // We should not make it `await`, as it never ends.
  await rolldownWatch({
    ...options,
    ...cliOptions.input,
  })
  logger.log(`Waiting for changes...`)
}

async function bundleInner(
  options: RolldownOptions,
  cliOptions: NormalizedCliOptions,
) {
  const startTime = performance.now()

  const build = await rolldown({ ...options, ...cliOptions.input })
  const bundleOutput = await build.write({
    ...options?.output,
    ...cliOptions.output,
  })

  const endTime = performance.now()

  printBundleOutputPretty(bundleOutput)

  logger.log(``)
  const duration = endTime - startTime
  // If the build time is more than 1s, we should display it in seconds.
  const spent =
    duration < 1000
      ? `${duration.toFixed(2)} ms`
      : `${(duration / 1000).toFixed(2)} s`
  logger.success(`Finished in ${colors.bold(spent)}`)
}

function printBundleOutputPretty(output: RolldownOutput) {
  const outputEntries = collectOutputEntries(output.output)
  const outputLayoutSizes = collectOutputLayoutAdjustmentSizes(outputEntries)
  printOutputEntries(outputEntries, outputLayoutSizes, '<DIR>')
}

type ChunkType = 'chunk' | 'asset'
type OutputEntry = {
  type: ChunkType
  fileName: string
  size: number
}

function collectOutputEntries(output: RollupOutput['output']): OutputEntry[] {
  return output.map((chunk) => ({
    type: chunk.type,
    fileName: chunk.fileName,
    size: chunk.type === 'chunk' ? chunk.code.length : chunk.source.length,
  }))
}

function collectOutputLayoutAdjustmentSizes(entries: OutputEntry[]) {
  let longest = 0
  let biggestSize = 0
  for (const entry of entries) {
    if (entry.fileName.length > longest) {
      longest = entry.fileName.length
    }
    if (entry.size > biggestSize) {
      biggestSize = entry.size
    }
  }

  const sizePad = displaySize(biggestSize).length

  return {
    longest,
    biggestSize,
    sizePad,
  }
}

const numberFormatter = new Intl.NumberFormat('en', {
  maximumFractionDigits: 2,
  minimumFractionDigits: 2,
})

function displaySize(bytes: number) {
  return `${numberFormatter.format(bytes / 1000)} kB`
}

const CHUNK_GROUPS = [
  { type: 'asset', color: 'green' },
  { type: 'chunk', color: 'cyan' },
] satisfies { type: ChunkType; color: keyof typeof colors }[]

function printOutputEntries(
  entries: OutputEntry[],
  sizeAdjustment: ReturnType<typeof collectOutputLayoutAdjustmentSizes>,
  distPath: string,
) {
  for (const group of CHUNK_GROUPS) {
    const filtered = entries.filter((e) => e.type === group.type)
    if (!filtered.length) {
      continue
    }
    for (const entry of filtered.sort((a, z) => a.size - z.size)) {
      // output format: `path/to/xxx type | size: y.yy kB`
      let log = colors.dim(withTrailingSlash(distPath))
      log += colors[group.color](
        entry.fileName.padEnd(sizeAdjustment.longest + 2),
      )
      log += colors.dim(entry.type)
      log += colors.dim(
        ` │ size: ${displaySize(entry.size).padStart(sizeAdjustment.sizePad)}`,
      )
      logger.log(log)
    }
  }
}

function withTrailingSlash(path: string): string {
  if (path[path.length - 1] !== '/') {
    return `${path}/`
  }
  return path
}
