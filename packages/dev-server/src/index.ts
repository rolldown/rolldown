import { RolldownOptions, rolldown } from 'rolldown'
import nodePath from 'node:path'
import connect from 'connect'
import http from 'node:http'
import sirv from 'sirv'

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
  ;(await rolldown(rolldownConfig)).write(rolldownConfig.output)

  const app = connect()

  console.log(`Serving ${nodePath.join(process.cwd(), 'dist')}`)
  app.use(sirv(nodePath.join(process.cwd(), 'dist')))

  //create node.js http server and listen on port
  http.createServer(app).listen(3000)
  console.log('Server listening on http://localhost:3000')
}
