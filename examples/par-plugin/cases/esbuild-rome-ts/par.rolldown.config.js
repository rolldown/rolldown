import { builtinModules } from 'node:module';
import nodePath from 'node:path';
import { defineConfig } from 'rolldown';
import { default as parallelPlugin } from '../../parallel-esbuild-plugin/index.js';

export const REPO_ROOT = nodePath.resolve(import.meta.dirname, '../../../..');

export default defineConfig({
  logLevel: 'silent',
  input: {
    rome: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts'),
  },
  external: builtinModules,
  // Need this due rome is not written with `isolatedModules: true`
  shimMissingExports: true,
  plugins: [parallelPlugin()],
  resolve: {
    extensions: ['.ts'],
    tsconfig: {
      configFile: nodePath.join(
        REPO_ROOT,
        './tmp/bench/rome/src/tsconfig.json',
      ),
    },
  },
});
