import { defineConfig } from 'rolldown'
import { builtinModules } from 'node:module'
import nodePath from 'node:path'
import nodeUrl from 'node:url'
import { default as parallelBabelPluginSync } from './parallel-babel-plugin/index.js'

const dirname = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url))

export const REPO_ROOT = nodePath.resolve(dirname, '../..')

export default defineConfig({
  logLevel: 'silent',
  input: {
    rome: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts'),
  },
  external: builtinModules,
  // Need this due rome is not written with `isolatedModules: true`
  shimMissingExports: true,
  plugins: [parallelBabelPluginSync()],
  resolve: {
    extensions: ['.ts'],
    tsconfigFilename: nodePath.join(
      REPO_ROOT,
      './tmp/bench/rome/src/tsconfig.json',
    ),
  },
  warmupFiles: [
    nodePath.join(REPO_ROOT, './tmp/bench/rome/src/rome/**/*.ts'),
    nodePath.join(REPO_ROOT, './tmp/bench/rome/src/@romejs/*/*.ts'),
    nodePath.join(REPO_ROOT, './tmp/bench/rome/src/@romejs/**/*.ts'),
  ],
  warmupFilesExclude: ['**/test-fixtures/**/*.ts', '**/*.test.ts'],
})
