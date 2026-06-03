import fs from 'node:fs'
import path from 'node:path'
import { defineTest } from 'rolldown-tests'
import { viteImportGlobPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [viteImportGlobPlugin()],
  },
  beforeTest() {
    // Create a directory with NFC-normalized name (single codepoint).
    // "ポ" (U+30DD) is the precomposed (NFC) form.
    const nfcDirName = '\u30DD' // "ポ" in NFC
    const dir = path.join(__dirname, nfcDirName)
    fs.mkdirSync(dir, { recursive: true })
    fs.writeFileSync(path.join(dir, 'a.js'), "export default 'a';\n")
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
