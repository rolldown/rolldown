// https://github.com/tinylibs/tinybench
import path from 'node:path'
import url from 'node:url'
import * as rolldown from '@rolldown/node'

const dirname = path.dirname(url.fileURLToPath(import.meta.url))

const build = await rolldown.rolldown({
  input: path.join(dirname, 'index.js'),
  // @ts-ignore
  cwd: dirname,
  resolve: {
    conditionNames: ['node', 'import'],
  },
})

await build.write()
