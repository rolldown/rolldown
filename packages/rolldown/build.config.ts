// build.config.ts
import { defineBuildConfig } from 'unbuild'
import nodeFs from 'node:fs'
import nodePath from 'node:path'
import { globSync } from 'glob'

export default defineBuildConfig({
  entries: [
    './src/index',
    {
      builder: 'rollup',
      input: './src/cli/index',
      name: 'cli',
    },
  ],
  clean: true,
  declaration: true, // generate .d.ts files
  externals: [/rolldown-binding\..*\.node/, /@rolldown\/binding-.*/],
  rollup: {
    emitCJS: true,
    cjsBridge: true,
    inlineDependencies: true,
  },
  hooks: {
    'build:done'(_ctx) {
      const binaryFiles = globSync('./src/rolldown-binding.*.node', {
        absolute: true,
      })
      // Binary build is on the separate step on CI
      if (!process.env.CI && binaryFiles.length === 0) {
        throw new Error('No binary files found')
      }
      // Move the binary file to dist
      binaryFiles.forEach((file) => {
        const fileName = nodePath.basename(file)
        console.log('Copying', file, 'to ./dist/shared')
        nodeFs.copyFileSync(file, `./dist/shared/${fileName}`)
      })
    },
  },
})
