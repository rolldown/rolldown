import { builtinModules } from 'node:module';
import nodePath from 'node:path';

import { ExternalOption } from 'rolldown';
import { babelPlugin } from '../parallel-babel-plugin/impl';
import { default as parallelBabelPlugin } from '../parallel-babel-plugin/index';
import type { BenchSuite } from '../types';
import { defineSuite, REPO_ROOT } from '../utils';

const inputs = [nodePath.join(REPO_ROOT, './tmp/bench/rome/src/entry.ts')];

const esbuildOptions = {
  tsconfig: nodePath.join(REPO_ROOT, './tmp/bench/rome/src/tsconfig.json'),
};

const rolldownOptionsForParallelism: BenchSuite['rolldownOptions'] = [
  {
    name: 'js-single',
    options: {
      logLevel: 'silent',
      external: builtinModules as ExternalOption,
      // Need this due rome is not written with `isolatedModules: true`
      shimMissingExports: true,
      plugins: [babelPlugin()],
      resolve: {
        extensions: ['.ts'],
        tsconfig: {
          configFile: nodePath.join(
            REPO_ROOT,
            './tmp/bench/rome/src/tsconfig.json',
          ),
        },
      },
    },
  },
  {
    name: 'js-parallel-babel',
    options: {
      logLevel: 'silent',
      external: builtinModules as ExternalOption,
      // Need this due rome is not written with `isolatedModules: true`
      shimMissingExports: true,
      plugins: [parallelBabelPlugin({})],
      resolve: {
        extensions: ['.ts'],
        tsconfig: {
          configFile: nodePath.join(
            REPO_ROOT,
            './tmp/bench/rome/src/tsconfig.json',
          ),
        },
      },
    },
  },
];

export const suiteRomeTsWithBabelAndParallelism = defineSuite({
  title: 'rome-ts-babel-parallelism',
  disableBundler: ['rollup', 'esbuild'],
  inputs,
  rolldownOptions: rolldownOptionsForParallelism,
});

export const suiteRomeTs = defineSuite({
  title: 'rome-ts',
  inputs,
  esbuildOptions,
  rolldownOptions: {
    logLevel: 'silent',
    external: builtinModules as ExternalOption,
    // Need this due rome is not written with `isolatedModules: true`
    shimMissingExports: true,
    resolve: {
      tsconfig: {
        configFile: nodePath.join(
          REPO_ROOT,
          './tmp/bench/rome/src/tsconfig.json',
        ),
      },
    },
  },
  disableBundler: ['rollup'],
  derived: {
    sourcemap: true,
  },
});
