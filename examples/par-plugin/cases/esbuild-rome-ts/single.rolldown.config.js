import { defineConfig } from 'rolldown'
import { builtinModules } from 'node:module'
import nodePath from 'node:path'
import { plugin } from '../../parallel-esbuild-plugin/impl.js'

export const REPO_ROOT = nodePath.resolve(import.meta.dirname, '../../../..')

export default defineConfig({
  logLevel: 'silent',
  input: {
    rome: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts'),
  },
  external: builtinModules,
  // Need this due rome is not written with `isolatedModules: true`
  shimMissingExports: true,
  plugins: [plugin()],
  resolve: {
    extensions: ['.ts'],
    tsconfigFilename: nodePath.join(
      REPO_ROOT,
      './tmp/bench/rome/src/tsconfig.json',
    ),
  },
})
