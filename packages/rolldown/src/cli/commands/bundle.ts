import path from 'node:path'
import { performance } from 'node:perf_hooks'
import { onExit } from 'signal-exit'
import { colors } from '../colors'
import { logger } from '../logger'
import { NormalizedCliOptions } from '../arguments/normalize'
import { arraify } from '../../utils/misc'
import { rolldown } from '../../api/rolldown'
import { watch as rolldownWatch } from '../../api/watch'
import type { RolldownOptions, RolldownOutput, RollupOutput } from '../..'
import { loadConfig } from '../load-config'

export async function bundleWithConfig(
  configPath: string,
  cliOptions: NormalizedCliOptions,
): Promise<void> {
  const config = await loadConfig(configPath)

  if (!config) {
    logger.error(`No configuration found at ${config}`)
    process.exit(1)
  }

  // TODO: Could add more validation/diagnostics here to emit a nice error message
  const configList = arraify(config)
  const operation = cliOptions.watch ? watchInner : bundleInner
  for (const config of configList) {
    await operation(config, cliOptions)
  }
}

export async function bundleWithCliOptions(
  cliOptions: NormalizedCliOptions,
): Promise<void> {
  if (cliOptions.output.dir) {
    const operation = cliOptions.watch ? watchInner : bundleInner
    await operation({}, cliOptions)
    return
  }

  if (cliOptions.watch) {
    logger.error('You must specify `output.dir` to use watch mode')
    process.exit(1)
  }

  // Rolldown doesn't yet support the following syntax:
  // await using build = await rolldown(cliOptions.input)
  const build = await rolldown(cliOptions.input)
  try {
    const { output: outputs } = await build.generate(cliOptions.output)

    if (outputs.length === 0) {
      logger.error('No output generated')
      process.exit(1)
    }

    for (const file of outputs) {
      if (outputs.length > 1) {
        logger.log(`\n${colors.cyan(colors.bold(`|→ ${file.fileName}:`))}\n`)
      }
      // avoid consola since it doesn't print it as raw string
      // eslint-disable-next-line no-console
      console.log(file.type === 'asset' ? file.source : file.code)
    }
  } finally {
    await build.close()
  }
}

async function watchInner(
  options: RolldownOptions,
  cliOptions: NormalizedCliOptions,
) {
  // Only if watch is true in CLI can we use watch mode.
  // We should not make it `await`, as it never ends.
  const watcher = await rolldownWatch({
    ...options,
    ...cliOptions.input,
    output: {
      ...options?.output,
      ...cliOptions.output,
    },
  })

  onExit((code: number | null | undefined) => {
    Promise.resolve(watcher.close()).finally(() => {
      process.exit(typeof code === 'number' ? code : 0)
    })
    return true
  })

  const changedFile: string[] = []
  watcher.on('change', (id, event) => {
    if (event.event === 'update') {
      changedFile.push(id)
    }
  })
  watcher.on('event', (event) => {
    switch (event.code) {
      case 'BUNDLE_START':
        if (changedFile.length > 0) {
          logger.log(
            `Found ${colors.bold(changedFile.map(relativeId).join(', '))} changed, rebuilding...`,
          )
        }
        changedFile.length = 0
        break

      case 'BUNDLE_END':
        logger.success(
          `Rebuilt ${colors.bold(relativeId(event.output[0]))} in ${colors.bold(ms(event.duration))}.`,
        )
        break

      case 'ERROR':
        logger.error(event.error)
        break

      default:
        break
    }
  })

  logger.log(`Waiting for changes...`)
}

async function bundleInner(
  options: RolldownOptions,
  cliOptions: NormalizedCliOptions,
) {
  const startTime = performance.now()

  const build = await rolldown({ ...options, ...cliOptions.input })
  try {
    const bundleOutput = await build.write({
      ...options?.output,
      ...cliOptions.output,
    })

    const endTime = performance.now()

    printBundleOutputPretty(bundleOutput)

    logger.log(``)
    const duration = endTime - startTime
    // If the build time is more than 1s, we should display it in seconds.
    logger.success(`Finished in ${colors.bold(ms(duration))}`)
  } finally {
    await build.close()
  }
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

function ms(duration: number) {
  return duration < 1000
    ? `${duration.toFixed(2)} ms`
    : `${(duration / 1000).toFixed(2)} s`
}

function relativeId(id: string): string {
  if (!path.isAbsolute(id)) return id
  return path.relative(path.resolve(), id)
}
