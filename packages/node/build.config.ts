// build.config.ts
import { defineBuildConfig } from 'unbuild'
import nodeFs from 'node:fs'
import nodePath from 'node:path'
import { globSync } from 'glob'

export default defineBuildConfig({
  entries: ['./src/index'],
  clean: true,
  declaration: true, // generate .d.ts files
  externals: [/rolldown-binding\..*\.node/],
  rollup: {
    emitCJS: true,
    cjsBridge: true,
  },
  hooks: {
    'build:done'(_ctx) {
      const binaryFiles = globSync('./src/rolldown-binding.*.node', {
        absolute: true,
      })
      if (binaryFiles.length === 0) {
        throw new Error('No binary files found')
      }
      // Move the binary file to dist
      binaryFiles.forEach((file) => {
        const fileName = nodePath.basename(file)
        console.log('Copying', file, 'to ./dist')
        nodeFs.copyFileSync(file, `./dist/${fileName}`)
      })
    },
  },
})
