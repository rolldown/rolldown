import { RolldownOptions, rolldown, Plugin } from 'rolldown'
import nodePath from 'node:path'
import connect from 'connect'
import serveStatic from 'serve-static'
import http from 'node:http'

interface DevConfig {}

async function loadConfig(): Promise<{
  rolldownConfig: RolldownOptions
  devConfig: DevConfig
}> {
  const exports = await import(nodePath.join(process.cwd(), 'rds.config.mjs'))
  return {
    rolldownConfig: exports.rolldownConfig,
    devConfig: exports.devConfig,
  }
}

export async function serve(): Promise<void> {
  console.log('Starting dev server...')
  const { rolldownConfig = {}, devConfig: _ = {} } = await loadConfig()

  if (Array.isArray(rolldownConfig.output)) {
    throw new Error('Multiple outputs are not supported in dev mode')
  }
  console.log(rolldownConfig)
  if (rolldownConfig.plugins == null || Array.isArray(rolldownConfig.plugins)) {
    rolldownConfig.plugins = [
      ...(rolldownConfig.plugins || []),
      createDevServerPlugin(),
    ]
  } else {
    throw new Error('Plugins must be an array')
  }

  ;(await rolldown(rolldownConfig)).write(rolldownConfig.output)

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

function createDevServerPlugin(): Plugin {
  return {
    name: 'rolldown-dev-server',
    generateBundle() {
      console.log('Generating index.html...')
      this.emitFile({
        type: 'asset',
        fileName: 'index.html',
        source: `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Vite + React + TS</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/main.js"></script>
  </body>
</html>
`,
      })
    },
  }
}
