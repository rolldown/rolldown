import { performance } from 'node:perf_hooks'
import { gzip } from 'node:zlib'
import { promisify } from 'node:util'
import consola from 'consola'
import { colors, ColorFunction } from 'consola/utils'
import { RolldownOptions, rolldown } from '../../index'
import { RolldownConfigExport } from '../../types/rolldown-config-export'
import { arraify } from '../../utils'
import { RollupOutput } from 'dist/index.mjs'

const compress = promisify(gzip)

export async function bundle(configExport: RolldownConfigExport) {
  const options = arraify(configExport)

  for (const option of options) {
    await bundleInner(option)
  }
}

async function bundleInner(options: RolldownOptions) {
  const dir = options.output?.dir ?? 'dist'

  const startTime = performance.now()

  const build = await rolldown(options)
  const _output = await build.write(options?.output)

  const entTime = performance.now()

  const outputEntries = await collectOutputEntries(_output.output)
  const outputLayoutSizes = collectOutputLayoutSizes(outputEntries)
  printOutputEntries(outputEntries, outputLayoutSizes, dir)

  consola.success(
    `Finished in ${colors.bold((entTime - startTime).toFixed(2))} ms`,
  )
}

type ChunkType = 'chunk' | 'asset'
type OutputEntry = {
  type: ChunkType
  fileName: string
  size: number
  compressedSize: number | null
}

async function collectOutputEntries(
  output: RollupOutput['output'],
): Promise<OutputEntry[]> {
  return await Promise.all(
    output.map(async (chunk) => {
      return {
        type: chunk.type,
        fileName: chunk.fileName,
        size: chunk.type === 'chunk' ? chunk.code.length : chunk.source.length,
        compressedSize: await calculateCompressedSize(
          chunk.type === 'chunk' ? chunk.code : chunk.source,
        ),
      }
    }),
  )
}

async function calculateCompressedSize(
  code: string | Uint8Array,
): Promise<number | null> {
  const compressed = await compress(
    typeof code === 'string' ? code : Buffer.from(code),
  )
  return compressed.length
}

function collectOutputLayoutSizes(entries: OutputEntry[]) {
  let longest = 0
  let biggestSize = 0
  let biggestCompressSize = 0
  for (const entry of entries) {
    if (entry.fileName.length > longest) {
      longest = entry.fileName.length
    }
    if (entry.size > biggestSize) {
      biggestSize = entry.size
    }
    if (entry.compressedSize && entry.compressedSize > biggestCompressSize) {
      biggestCompressSize = entry.compressedSize
    }
  }

  const sizePad = displaySize(biggestSize).length
  const compressPad = displaySize(biggestCompressSize).length

  return {
    longest,
    biggestSize,
    biggestCompressSize,
    sizePad,
    compressPad,
  }
}

const numberFormatter = new Intl.NumberFormat('en', {
  maximumFractionDigits: 2,
  minimumFractionDigits: 2,
})

const displaySize = (bytes: number) => {
  return `${numberFormatter.format(bytes / 1000)} kB`
}

const CHUNK_GROUPS = [
  { type: 'asset', color: colors.green },
  { type: 'chunk', color: colors.cyan },
] satisfies { type: ChunkType; color: ColorFunction }[]

function printOutputEntries(
  entries: OutputEntry[],
  sizeAdjustment: ReturnType<typeof collectOutputLayoutSizes>,
  distPath: string,
) {
  for (const group of CHUNK_GROUPS) {
    const filtered = entries.filter((e) => e.type === group.type)
    if (!filtered.length) {
      continue
    }
    for (const entry of filtered.sort((a, z) => a.size - z.size)) {
      // format: `path/to/xxxx type | size: y.yy kB | gzip: z.zz kB`
      let log = colors.dim(withTrailingSlash(distPath))
      log += group.color(entry.fileName.padEnd(sizeAdjustment.longest + 2))
      log += colors.white(entry.type)
      log += colors.dim(
        ` │ size: ${displaySize(entry.size).padStart(sizeAdjustment.sizePad)}`,
      )
      if (entry.compressedSize) {
        log += colors.dim(
          ` │ gzip: ${displaySize(entry.compressedSize).padStart(
            sizeAdjustment.compressPad,
          )}`,
        )
      }
      consola.info(log)
    }
  }
}

function withTrailingSlash(path: string): string {
  if (path[path.length - 1] !== '/') {
    return `${path}/`
  }
  return path
}
