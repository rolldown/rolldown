import { defineConfig } from 'rolldown'
import esbuild from 'esbuild'
import nodePath from 'node:path'
import nodeFs from 'node:fs'
import { $ } from 'execa'
import { globSync } from 'glob'

const outputDir = 'dist/dist'

export default defineConfig({
  input: {
    index: './src/index',
    cli: './src/cli/index',
    'parallel-plugin': './src/parallel-plugin',
    'parallel-plugin-worker': './src/parallel-plugin-worker',
  },
  platform: 'node',
  external: [
    /rolldown-binding\..*\.node/,
    /rolldown-binding\..*\.wasm/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
  ],
  output: {
    dir: outputDir,
    format: 'esm',
    entryFileNames: '[name].mjs',
    chunkFileNames: 'shared/[name].mjs',
    // Cjs shims for esm format
    banner: [
      `import __node_module__ from 'node:module';`,
      `const require = __node_module__.createRequire(import.meta.url)`,
    ].join('\n'),
  },
  resolve: {
    extensions: ['.ts', '.js'],
    alias: {
      '@src': nodePath.resolve(import.meta.dirname, 'src'),
    },
  },
  plugins: [
    {
      name: 'shim',
      async buildStart() {
        await $({
          cwd: import.meta.dirname,
          stdin: 'inherit',
          stderr: 'inherit',
          stdout: 'inherit',
        })`pnpm build-binding`
      },
      async transform(code, id) {
        if (id.endsWith('.ts')) {
          const ret = await esbuild.transform(code, { loader: 'ts' })
          return ret.code
        }
      },
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

        // Move the binary file to dist
        binaryFiles.forEach((file) => {
          const fileName = nodePath.basename(file)
          console.log('[build:done] Copying', file, `to ${copyTo}`)
          nodeFs.copyFileSync(file, nodePath.join(copyTo, fileName))
          console.log(`[build:done] Cleaning ${file}`)
          nodeFs.rmSync(file)
        })
        wasiShims.forEach((file) => {
          const fileName = nodePath.basename(file)
          console.log('[build:done] Copying', file, 'to ./dist/shared')
          nodeFs.copyFileSync(file, nodePath.join(copyTo, fileName))
        })
      },
    },
  ],
})
