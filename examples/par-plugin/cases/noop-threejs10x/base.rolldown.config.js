import { builtinModules } from 'node:module';
import nodePath from 'node:path';
import { defineConfig } from 'rolldown';

export const REPO_ROOT = nodePath.resolve(import.meta.dirname, '../../../..');

export default defineConfig({
  logLevel: 'silent',
  input: {
    three: nodePath.join(REPO_ROOT, './tmp/bench/three10x/entry.js'),
  },
  external: builtinModules,
  plugins: [],
});
