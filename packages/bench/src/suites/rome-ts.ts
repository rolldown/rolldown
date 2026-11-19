import { builtinModules } from 'node:module';
import nodePath from 'node:path';

import type { ExternalOption } from 'rolldown';
import { defineSuite, REPO_ROOT } from '../utils';

const inputs = [nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts')];

const esbuildOptions = {
  tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
};

export const suiteRomeTs = defineSuite({
  title: 'rome-ts',
  inputs,
  esbuildOptions,
  rolldownOptions: {
    logLevel: 'silent',
    external: builtinModules as ExternalOption,
    // Need this due rome is not written with `isolatedModules: true`
    shimMissingExports: true,
    tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
  },
  disableBundler: ['rollup'],
  derived: {
    sourcemap: true,
  },
});
