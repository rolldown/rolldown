import { performance } from 'node:perf_hooks'
import {
  RolldownOptions,
  RolldownOutput,
  RollupOutput,
  rolldown,
} from '../../index.js'
import { arraify } from '../../utils/index.js'
import { ensureConfig, logger } from '../utils.js'
import * as colors from '../colors.js'

export async function bundle(configPath: string) {
  const config = await ensureConfig(configPath)

  if (!config) {
    logger.error(`No configuration found at ${config}`)
    process.exit(1)
  }

  const configList = arraify(config)

  for (const config of configList) {
    await bundleInner(config)
  }
}

async function bundleInner(options: RolldownOptions) {
  const startTime = performance.now()

  const build = await rolldown(options)
  const bundleOutput = await build.write(options?.output)

  const endTime = performance.now()

  printBundleOutputPretty(bundleOutput)

  logger.log(``)
  logger.success(
    `Finished in ${colors.bold((endTime - startTime).toFixed(2))} ms`,
  )
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
