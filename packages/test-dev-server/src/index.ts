import { build } from 'rolldown'
import nodePath from 'node:path'
import connect from 'connect'
import serveStatic from 'serve-static'
import http from 'node:http'
import { DevConfig, defineDevConfig } from './define-dev-config.js'
import { createDevServerPlugin } from './create-dev-server-plugin.js'

async function loadDevConfig(): Promise<DevConfig> {
  const exports = await import(nodePath.join(process.cwd(), 'dev.config.mjs'))
  return exports.default
}

export async function serve(): Promise<void> {
  console.log('Starting dev server...')
  const devConfig = await loadDevConfig()
  const buildOptions = devConfig.build ?? {}
  if (buildOptions.plugins == null || Array.isArray(buildOptions.plugins)) {
    buildOptions.plugins = [
      ...(buildOptions.plugins || []),
      createDevServerPlugin(),
    ]
  } else {
    throw new Error('Plugins must be an array')
  }
  buildOptions.write = true

  await build(buildOptions)

  const app = connect()

  console.log(`Serving ${nodePath.join(process.cwd(), 'dist')}`)
  app.use(
    serveStatic(nodePath.join(process.cwd(), 'dist'), {
      index: ['index.html'],
      extensions: ['html'],
    }),
  )

  //create node.js http server and listen on port
  http.createServer(app).listen(3000)
  console.log('Server listening on http://localhost:3000')
}

export { defineDevConfig }
