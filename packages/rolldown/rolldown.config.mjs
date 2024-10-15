// @ts-check

import { defineConfig } from 'npm-rolldown'
import pkgJson from './package.json' with { type: 'json' }
import nodePath from 'node:path'
import fsExtra from 'fs-extra'
import { globSync } from 'glob'

const outputDir = 'dist'

const shared = defineConfig({
  input: {
    index: './src/index',
    cli: './src/cli/index',
    'parallel-plugin': './src/parallel-plugin',
    'parallel-plugin-worker': './src/parallel-plugin-worker',
    'experimental-index': './src/experimental-index',
  },
  platform: 'node',
  external: [
    /rolldown-binding\..*\.node/,
    /rolldown-binding\..*\.wasm/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
    // some dependencies, e.g. zod, cannot be inlined because their types
    // are used in public APIs
    ...Object.keys(pkgJson.dependencies),
  ],
})

export default defineConfig([
  {
    ...shared,
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: 'esm/[name].mjs',
      chunkFileNames: 'shared/[name]-[hash].mjs',
      // Cjs shims for esm format
      banner: [
        `import __node_module__ from 'node:module';`,
        `const require = __node_module__.createRequire(import.meta.url)`,
      ].join('\n'),
    },
    plugins: [
      {
        name: 'shim',
        buildEnd() {
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

          const copyTo = nodePath.resolve(outputDir, 'shared')
          fsExtra.ensureDirSync(copyTo)

          // Move the binary file to dist
          binaryFiles.forEach((file) => {
            const fileName = nodePath.basename(file)
            console.log('[build:done] Copying', file, `to ${copyTo}`)
            fsExtra.copyFileSync(file, nodePath.join(copyTo, fileName))
            console.log(`[build:done] Cleaning ${file}`)
            fsExtra.rmSync(file)
          })
          wasiShims.forEach((file) => {
            const fileName = nodePath.basename(file)
            console.log('[build:done] Copying', file, 'to ./dist/shared')
            fsExtra.copyFileSync(file, nodePath.join(copyTo, fileName))
          })

          // Move watcher-worker file to dist
          const fileName = 'watcher-worker.js'
          console.log('[build:done] Copying', fileName, 'to ./dist/shared')
          fsExtra.copyFileSync(
            nodePath.join('./src', fileName),
            nodePath.join(copyTo, fileName),
          )

          // Copy binding types and rollup types to dist
          const distTypesDir = nodePath.resolve(outputDir, 'types')
          fsExtra.ensureDirSync(distTypesDir)
          const types = globSync(['./src/*.d.ts'], {
            absolute: true,
          })
          types.forEach((file) => {
            const fileName = nodePath.basename(file)
            console.log('[build:done] Copying', file, 'to ./dist/shared')
            fsExtra.copyFileSync(file, nodePath.join(distTypesDir, fileName))
          })
        },
      },
    ],
  },
  {
    ...shared,
    plugins: [
      {
        name: 'shim-import-meta',
        transform(code, id) {
          if (id.endsWith('.ts') && code.includes('import.meta.resolve')) {
            return code.replace('import.meta.resolve', 'undefined')
          }
        },
      },
    ],
    output: {
      dir: outputDir,
      format: 'cjs',
      entryFileNames: 'cjs/[name].cjs',
      chunkFileNames: 'shared/[name]-[hash].cjs',
    },
  },
])
