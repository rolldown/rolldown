import { defineConfig } from 'npm-rolldown'
import pkgJson from './package.json' with { type: 'json' }
import esbuild from 'esbuild'
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
  },
  platform: 'node',
  external: [
    /rolldown-binding\..*\.node/,
    /rolldown-binding\..*\.wasm/,
    /@rolldown\/binding-.*/,
    /\.\/rolldown-binding\.wasi\.cjs/,
    ...Object.keys(pkgJson.dependencies).filter(
      (dep) =>
        // `locate-character` only exports esm, so we have to inline it.
        dep !== 'locate-character',
    ),
  ],
  resolve: {
    extensions: ['.ts', '.js'],
    alias: {
      '@src': nodePath.resolve(import.meta.dirname, 'src'),
    },
  },
})

/**
 * @returns {import('npm-rolldown').Plugin[]}
 */
const sharedPlugins = (outputCjs = false) => [
  {
    name: 'transpile-ts',
    async transform(code, id) {
      if (id.endsWith('.ts')) {
        const ret = await esbuild.transform(code, {
          loader: 'ts',
          define: outputCjs
            ? {
                'import.meta.resolve': 'undefined',
              }
            : {},
        })
        return ret.code
      }
    },
  },
]

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
      ...sharedPlugins(),
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
        },
      },
    ],
  },
  {
    ...shared,
    plugins: [...sharedPlugins(true)],
    output: {
      dir: outputDir,
      format: 'cjs',
      entryFileNames: 'cjs/[name].cjs',
      chunkFileNames: 'shared/[name]-[hash].cjs',
    },
  },
])
