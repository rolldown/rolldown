// build.config.ts
import { globSync } from 'glob'
import nodeFs from 'node:fs'
import nodePath from 'node:path'
import { defineBuildConfig } from 'unbuild'

export default defineBuildConfig({
  entries: [
    './src/index',
    {
      builder: 'rollup',
      input: './src/cli/index',
      name: 'cli',
    },
  ],
  sourcemap: true,
  clean: true,
  declaration: true, // generate .d.ts files
  externals: [/rolldown-binding\..*\.node/, /@rolldown\/binding-.*/],
  rollup: {
    emitCJS: true,
    cjsBridge: true,
    inlineDependencies: true,
    resolve: {
      exportConditions: ['node'],
    },
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
