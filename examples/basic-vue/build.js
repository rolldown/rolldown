// https://github.com/tinylibs/tinybench
import path from 'node:path'
import url from 'url'
import * as rolldown from '@rolldown/node'

const dirname = path.dirname(url.fileURLToPath(import.meta.url))

const build = await rolldown.rolldown({
  input: path.join(dirname, 'index.js'),
  cwd: dirname,
})

await build.write()
