// cSpell:disable
import { defineTest } from '@tests'
import { expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export default defineTest({
  config: {
    output: {
      // change directory structure to test relative path resolution
      entryFileNames: './entries/[name].mjs',
      assetFileNames: './assets/[name]-[hash].[ext]',
    },
    plugins: [
      // example plugin from
      // https://rollupjs.org/plugin-development/#file-urls
      {
        name: "svg-resolver",
        resolveId(source, importer) {
          if (source.endsWith(".svg")) {
            return path.resolve(path.dirname(importer), source);
          }
        },
        load(id) {
          if (id.endsWith(".svg")) {
            const referenceId = this.emitFile({
              type: "asset",
              name: path.basename(id),
              source: fs.readFileSync(id),
            });
            return `export default import.meta.ROLLUP_FILE_URL_${referenceId};`;
          }
        },
      },
    ],
  },
  afterTest: async () => {
    const mod = await import("./dist/entries/main.mjs");
    const emitted = fs.readFileSync(fileURLToPath(mod.default), "utf-8");
    const original = fs.readFileSync(path.join(import.meta.dirname, "./main.svg"), "utf-8");
    expect(emitted).toBe(original)
  },
})
