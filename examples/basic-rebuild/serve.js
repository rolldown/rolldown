// @ts-check
import { defineConfig, rolldown } from 'rolldown'
import fs from 'node:fs'

const config = defineConfig({
  input: {
    entry: './src/index.js',
  },
  output: {
    dir: './dist',
  },
  plugins: [],
})

/**
 * @param {string} filepath
 * @param {(s: string) => string} editFn
 */
function edit(filepath, editFn) {
  fs.writeFileSync(filepath, editFn(fs.readFileSync(filepath, 'utf-8')))
}

async function main() {
  const build = await rolldown(config)
  const output1 = await build.write(config.output)
  console.log(output1.output)
  edit('./src/dep.js', (s) =>
    s.replace(/true|false/, (m) => (m === 'true' ? 'false' : 'true')),
  )
  const output2 = await build.rebuild()
  console.log(output2.output)
}

main()
