import * as rolldown from 'rolldown'
import nodePath from 'node:path'
import connect from 'connect'
import serveStatic from 'serve-static'
import http from 'node:http'
import chokidar from 'chokidar'
import nodeFs from 'node:fs'
import { WebSocketServer, WebSocket } from 'ws'
import { DevConfig, defineDevConfig } from './define-dev-config.js'
import { createDevServerPlugin } from './create-dev-server-plugin.js'

let seed = 0

async function loadDevConfig(): Promise<DevConfig> {
  const exports = await import(nodePath.join(process.cwd(), 'dev.config.mjs'))
  return exports.default
}

class DevServer {
  private config: DevConfig

  constructor(config: DevConfig) {
    this.config = config
  }

  async serve() {
    const buildOptions = this.config.build ?? {}
    if (buildOptions.plugins == null || Array.isArray(buildOptions.plugins)) {
      buildOptions.plugins = [
        ...(buildOptions.plugins || []),
        createDevServerPlugin(),
      ]
    } else {
      throw new Error('Plugins must be an array')
    }
    // buildOptions.write = true
    console.log('Build options:', buildOptions)
    // const build = await rolldown.build(buildOptions)
    const build = await rolldown.rolldown(buildOptions)
    await build.write(buildOptions.output)

    const app = connect()

    console.log(`Serving ${nodePath.join(process.cwd(), 'dist')}`)
    const watcher = chokidar.watch(nodePath.join(process.cwd(), 'src'))

    app.use(
      serveStatic(nodePath.join(process.cwd(), 'dist'), {
        index: ['index.html'],
        extensions: ['html'],
      }),
    )

    //create node.js http server and listen on port
    const server = http.createServer(app)
    const wsServer = new WebSocketServer({ server })
    let socket: WebSocket
    wsServer.on('connection', function connection(ws) {
      socket = ws
      console.log(`Ws connected`)
      ws.on('error', console.error)
    })
    watcher.on('change', async (path) => {
      console.log(`File ${path} has been changed`)
      const patch = await build.generateHmrPatch([path])
      if (patch) {
        console.log('Patching...')
        if (socket) {
          const path = `${seed}.js`
          seed++
          nodeFs.writeFileSync(
            nodePath.join(process.cwd(), 'dist', path),
            patch,
          )
          console.log(
            'Patch:',
            JSON.stringify({
              type: 'update',
              url: path,
            }),
          )
          socket.send(
            JSON.stringify({
              type: 'update',
              url: path,
            }),
          )
        } else {
          console.log('No socket connected')
        }
      } else {
        console.log('No patch found')
      }
    })
    server.listen(3000)
    console.log('Server listening on http://localhost:3000')
  }
}

export async function serve(): Promise<void> {
  console.log('Starting dev server...')
  const devConfig = await loadDevConfig()
  const devServer = new DevServer(devConfig)
  await devServer.serve()
}

export { defineDevConfig }
