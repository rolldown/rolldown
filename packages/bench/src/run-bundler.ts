import commonjs from '@rollup/plugin-commonjs';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import * as esbuild from 'esbuild';
import path from 'node:path';
import * as rolldown from 'rolldown';
import * as rollup from 'rollup';

import type { BenchSuite, RolldownBenchSuite } from './types';
import { PROJECT_ROOT } from './utils.js';

export function getRolldownSuiteList(suite: BenchSuite): RolldownBenchSuite[] {
  const rolldownOptionsList = Array.isArray(suite.rolldownOptions)
    ? suite.rolldownOptions
    : [{ name: 'default', options: suite.rolldownOptions }];
  return rolldownOptionsList.map(({ name, options }) => ({
    suiteName: name,
    title: suite.title,
    inputs: suite.inputs,
    options,
  }));
}

export async function runRolldown(suite: RolldownBenchSuite) {
  const { output: outputOptions = {}, ...inputOptions } = suite.options ?? {};
  const build = await rolldown.rolldown({
    platform: 'node',
    input: suite.inputs,
    ...inputOptions,
  });
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rolldown/${suite.title}`),
    ...outputOptions,
  });
  await build.close();
}

export async function runEsbuild(suite: BenchSuite) {
  const options = suite.esbuildOptions ?? {};
  await esbuild.build({
    platform: 'node',
    entryPoints: suite.inputs,
    bundle: true,
    outdir: path.join(PROJECT_ROOT, `./dist/esbuild/${suite.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    ...options,
  });
}

export async function runRollup(suite: BenchSuite) {
  const { output: outputOptions = {}, ...inputOptions } = suite.rollupOptions ??
    {};
  const build = await rollup.rollup({
    input: suite.inputs,
    onwarn: (_warning, _defaultHandler) => {
      // ignore warnings
    },
    plugins: [
      nodeResolve({
        exportConditions: ['import'],
        mainFields: ['module', 'browser', 'main'],
      }),
      commonjs(),
    ],
    ...inputOptions,
  });
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${suite.title}`),
    ...outputOptions,
  });
}
