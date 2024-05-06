// build.config.ts
import { globSync } from 'glob'
import nodeFs from 'node:fs'
import nodeUrl from 'node:url'
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
    './src/parallel-plugin',
    './src/parallel-plugin-worker',
  ],
  alias: {
    '@src': nodePath.resolve(
      nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url)),
      'src',
    ),
  },
  sourcemap: true,
  clean: true,
  declaration: true, // generate .d.ts files
  externals: [
    /rolldown-binding\..*\.node/,
    /rolldown-binding\..*\.wasm/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
  ],
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
      const binaryFiles = globSync(
        ['./src/rolldown-binding.*.node', './src/rolldown-binding.*.wasm'],
        {
          absolute: true,
        },
      )
      const wasiShims = globSync(
        ['./src/*.wasi.js', './src/*.wasi.cjs', './src/*.mjs'],
        {
          absolute: true,
        },
      )
      // Binary build is on the separate step on CI
      if (!process.env.CI && binaryFiles.length === 0) {
        throw new Error('No binary files found')
      }

      // Move the binary file to dist
      binaryFiles.forEach((file) => {
        const fileName = nodePath.basename(file)
        console.log('[build:done] Copying', file, 'to ./dist/shared')
        nodeFs.copyFileSync(file, `./dist/shared/${fileName}`)
        console.log(`[build:done] Cleaning ${file}`)
        nodeFs.rmSync(file)
      })
      wasiShims.forEach((file) => {
        const fileName = nodePath.basename(file)
        console.log('[build:done] Copying', file, 'to ./dist/shared')
        nodeFs.copyFileSync(file, `./dist/shared/${fileName}`)
      })
    },
  },
})
