import { builtinModules } from 'node:module';
import nodePath from 'node:path';
import { defineConfig } from 'rolldown';
import { noopPlugin } from '../../parallel-noop-plugin/impl.js';

export const REPO_ROOT = nodePath.resolve(import.meta.dirname, '../../../..');

export default defineConfig({
  logLevel: 'silent',
  input: {
    three: nodePath.join(REPO_ROOT, './tmp/bench/three10x/entry.js'),
  },
  external: builtinModules,
  plugins: [noopPlugin()],
});
