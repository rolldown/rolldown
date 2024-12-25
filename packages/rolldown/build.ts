import { defineConfig, OutputOptions, rolldown } from './src/index'
import pkgJson from './package.json' with { type: 'json' }
import nodePath from 'node:path'
import fsExtra from 'fs-extra'
import { globSync } from 'glob'

const outputDir = 'dist'

const IS_RELEASING_CI = !!process.env.RELEASING

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

const configs = defineConfig([
  {
    ...shared,
    output: {
      dir: outputDir,
      format: 'esm',
      entryFileNames: 'esm/[name].mjs',
      chunkFileNames: 'shared/[name]-[hash].mjs',
    },
    plugins: [
      {
        name: 'shim',
        buildEnd() {
          // wasm build rely on `.node` binaries. But we don't want to copy `.node` files
          // to the dist folder, so we need to distinguish between `.wasm` and `.node` files.
          const wasmFiles = globSync(['./src/rolldown-binding.*.wasm'], {
            absolute: true,
          })

          const isWasmBuild = wasmFiles.length > 0

          const nodeFiles = globSync(['./src/rolldown-binding.*.node'], {
            absolute: true,
          })

          const wasiShims = globSync(
            ['./src/*.wasi.js', './src/*.wasi.cjs', './src/*.mjs'],
            {
              absolute: true,
            },
          )
          // Binary build is on the separate step on CI
          if (
            !process.env.CI &&
            wasmFiles.length === 0 &&
            nodeFiles.length === 0
          ) {
            throw new Error('No binary files found')
          }

          const copyTo = nodePath.resolve(outputDir, 'shared')
          fsExtra.ensureDirSync(copyTo)

          if (!IS_RELEASING_CI) {
            // Released `rolldown` package import binary via `@rolldown/binding-<platform>` packages.
            // There's no need to copy binary files to dist folder.

            if (isWasmBuild) {
              // Move the binary file to dist
              wasmFiles.forEach((file) => {
                const fileName = nodePath.basename(file)
                console.log('[build:done] Copying', file, `to ${copyTo}`)
                fsExtra.copyFileSync(file, nodePath.join(copyTo, fileName))
                console.log(`[build:done] Cleaning ${file}`)
                try {
                  // GitHub windows runner emits `operation not permitted` error, most likely because of the file is still in use.
                  // We could safely ignore the error.
                  fsExtra.rmSync(file)
                } catch {}
              })
            } else {
              // Move the binary file to dist
              nodeFiles.forEach((file) => {
                const fileName = nodePath.basename(file)
                console.log('[build:done] Copying', file, `to ${copyTo}`)
                fsExtra.copyFileSync(file, nodePath.join(copyTo, fileName))
                console.log(`[build:done] Cleaning ${file}`)
                try {
                  fsExtra.rmSync(file)
                } catch {}
              })
            }

            wasiShims.forEach((file) => {
              const fileName = nodePath.basename(file)
              console.log('[build:done] Copying', file, 'to ./dist/shared')
              fsExtra.copyFileSync(file, nodePath.join(copyTo, fileName))
            })
          }

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

      {
        name: 'cleanup binding.js',
        transform: {
          filter: {
            code: {
              include: ['require = createRequire(__filename)'],
            },
          },
          handler(code, id) {
            if (id.endsWith('binding.js')) {
              const ret = code.replace(
                'require = createRequire(__filename)',
                '',
              )
              return ret
            }
          },
        },
      },
    ],
  },
  {
    ...shared,
    plugins: [
      {
        name: 'shim-import-meta',
        transform: {
          filter: {
            code: {
              include: ['import.meta.resolve'],
            },
          },
          handler(code, id) {
            if (id.endsWith('.ts') && code.includes('import.meta.resolve')) {
              return code.replace('import.meta.resolve', 'undefined')
            }
          },
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

;(async () => {
  for (const config of configs) {
    await (await rolldown(config)).write(config.output as OutputOptions)
  }
})()
