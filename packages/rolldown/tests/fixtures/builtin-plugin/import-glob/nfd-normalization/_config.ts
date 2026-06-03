import fs from 'node:fs'
import path from 'node:path'
import { defineTest } from 'rolldown-tests'
import { viteImportGlobPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [viteImportGlobPlugin()],
  },
  beforeTest() {
    // Create a directory with NFD-normalized name to simulate macOS filesystem behavior.
    // "ポ" (U+30DD) decomposes to "ホ" (U+30DB) + combining handakuten (U+309A)
    const nfdDirName = '\u30DB\u309A' // "ポ" in NFD
    const dir = path.join(__dirname, nfdDirName)
    fs.mkdirSync(dir, { recursive: true })
    fs.writeFileSync(path.join(dir, 'a.js'), "export default 'a';\n")
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
