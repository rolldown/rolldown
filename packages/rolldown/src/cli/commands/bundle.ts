import { performance } from 'node:perf_hooks'
import { rolldown } from '../../rolldown'
import type { RolldownOptions, RolldownOutput, RollupOutput } from '../../index'
import { arraify } from '../../utils/misc'
import { ensureConfig, logger } from '../utils'
import * as colors from '../colors'
import { NormalizedCliOptions } from '../arguments/normalize'
import { createServer } from 'node:http'
import { WebSocketServer, WebSocket } from 'ws'
import chokidar from 'chokidar'
import connect from 'connect'
import path from 'node:path'

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
    await bundleInner(config, cliOptions)
  }
}

export async function bundleWithCliOptions(cliOptions: NormalizedCliOptions) {
  // TODO when supports `output.file`, we should modify it here.
  if (cliOptions.output.dir) {
    await bundleInner({}, cliOptions)
  } else {
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
  }
}

async function bundleInner(
  options: RolldownOptions,
  cliOptions: NormalizedCliOptions,
) {
  const startTime = performance.now()

  const build = await rolldown({ ...options, ...cliOptions.input })
  const bundleOutput = options.dev
    ? await build.generate({
        ...options?.output,
        ...cliOptions.output,
      })
    : await build.write({
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

  if (options.dev) {
    const cwd = options.cwd ?? process.cwd()
    const outputDir = options.output?.dir ?? 'dist'
    const app = connect()
    const virtualFiles = Object.fromEntries(
      bundleOutput.output.map((chunk) => [
        chunk.fileName,
        chunk.type === 'chunk' ? chunk.code : chunk.source,
      ]),
    )
    app.use((req, res, next) => {
      if (req.url) {
        const url = req.url === '/' ? 'index.html' : req.url.slice(1)
        const virtualFile = virtualFiles[url]
        if (virtualFile) {
          res.end(virtualFile)
        } else {
          console.log(req.url + 'not found')
          next()
        }
      }
    })
    const server = createServer(app)
    const wsServer = new WebSocketServer({ server })
    let socket: WebSocket
    wsServer.on('connection', function connection(ws) {
      socket = ws
      logger.log(`Ws connected`)
      ws.on('error', console.error)
    })
    logger.log(`Dev server running at`, colors.cyan('http://localhost:8080'))

    server.listen(8080)

    logger.log(`Watching for changes...`)
    const watcher = chokidar.watch([cwd], {
      ignored: [
        '**/.git/**',
        '**/node_modules/**',
        '**/test-results/**',
        path.join(cwd, outputDir),
      ],
      ignoreInitial: true,
      ignorePermissionErrors: true,
      // for windows and macos, we need to wait for the file to be written
      awaitWriteFinish:
        process.platform === 'linux'
          ? undefined
          : {
              stabilityThreshold: 10,
              pollInterval: 10,
            },
    })
    watcher.on('change', async (file) => {
      if (file) {
        logger.log(`Found change in ${file}`)
        const [fileName, content] = await build.experimental_hmr_rebuild([file])
        virtualFiles[fileName] = content
        if (socket) {
          socket.send(
            JSON.stringify({
              type: 'update',
              url: fileName,
            }),
          )
        }
      }
    })
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
