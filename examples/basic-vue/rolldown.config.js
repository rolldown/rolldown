import { defineConfig } from 'rolldown'
import nodePath from 'path'
import nodeFs from 'node:fs'

export default defineConfig({
  input: './index.js',
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  // plugins: [
  //   {
  //     name: 'resolve',
  //     resolveId(id, importer) {
  //       let dir = importer ? nodePath.dirname(importer) : process.cwd()
  //       let p = nodePath.resolve(dir, id)
  //       if (nodeFs.existsSync(p)) {
  //         return p
  //       }
  //     },
  //     load(id) {
  //       return nodeFs.readFileSync(id, 'utf-8')
  //     },
  //   },
  // ],
})
