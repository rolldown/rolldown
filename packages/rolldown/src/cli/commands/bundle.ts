import { performance } from 'node:perf_hooks'
import { colors, ColorFunction } from 'consola/utils'
import { RolldownOptions, RollupOutput, rolldown } from '../../index.js'
import { RolldownConfigExport } from '../../types/rolldown-config-export.js'
import { arraify } from '../../utils/index.js'
import { logger } from '../utils.js'
import { brandColor } from '../colors.js'
import { name, version } from '../../../package.json' assert { type: 'json' }

export async function bundle(configExport: RolldownConfigExport) {
  const options = arraify(configExport)

  for (const option of options) {
    await bundleInner(option)
  }
}

async function bundleInner(options: RolldownOptions) {
  const dir = options.output?.dir ?? 'dist'

  logger.log(`${brandColor(`${name} ${version}`)}: bundling ...\n`)

  const startTime = performance.now()

  const build = await rolldown(options)
  const _output = await build.write(options?.output)

  const entTime = performance.now()

  const outputEntries = collectOutputEntries(_output.output)
  const outputLayoutSizes = collectOutputLayoutAdjustmentSizes(outputEntries)
  printOutputEntries(outputEntries, outputLayoutSizes, dir)

  logger.log(``)
  logger.success(
    `Finished in ${colors.bold((entTime - startTime).toFixed(2))} ms`,
  )
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
  { type: 'asset', color: colors.green },
  { type: 'chunk', color: colors.cyan },
] satisfies { type: ChunkType; color: ColorFunction }[]

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
      log += group.color(entry.fileName.padEnd(sizeAdjustment.longest + 2))
      log += colors.white(entry.type)
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
