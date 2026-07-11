import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';

const suiteName = process.argv[2];
const outputPath = process.argv[3];
if (!suiteName || !outputPath) {
  throw new Error('usage: node run-wall-time-matrix.mjs <suite> <hyperfine-output.json>');
}

const nodeBinary = process.execPath;
const cliPath = nodePath.join(import.meta.dirname, 'node_modules/rolldown/bin/cli.mjs');
const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;

const variant = (name, configPath, workerCount) => ({
  name,
  command: [
    ...(workerCount === undefined
      ? []
      : ['/usr/bin/env', `ROLLDOWN_PARALLEL_PLUGIN_WORKERS=${workerCount}`]),
    nodeBinary,
    cliPath,
    '-c',
    nodePath.join(import.meta.dirname, configPath),
  ]
    .map(quote)
    .join(' '),
});

const suites = {
  'noop-five': {
    warmup: 1,
    runs: 5,
    variants: [
      variant('ordinary', 'cases/noop-threejs10x/single-noop.rolldown.config.js'),
      ...[1, 2, 4, 8].map((count) =>
        variant(`worker-${count}`, 'cases/noop-threejs10x/par-noop.rolldown.config.js', count),
      ),
    ],
  },
  'babel-five': {
    warmup: 1,
    runs: 5,
    variants: [
      variant('ordinary', 'cases/babel-rome-ts/single-babel.rolldown.config.js'),
      ...[1, 2, 4, 8].map((count) =>
        variant(`worker-${count}`, 'cases/babel-rome-ts/par-babel.rolldown.config.js', count),
      ),
    ],
  },
  'babel-confirm-control': {
    warmup: 2,
    runs: 10,
    variants: [
      variant('ordinary', 'cases/babel-rome-ts/single-babel.rolldown.config.js'),
      variant('worker-1', 'cases/babel-rome-ts/par-babel.rolldown.config.js', 1),
    ],
  },
  'babel-confirm-workers': {
    warmup: 2,
    runs: 10,
    variants: [2, 4, 8].map((count) =>
      variant(`worker-${count}`, 'cases/babel-rome-ts/par-babel.rolldown.config.js', count),
    ),
  },
};

const suite = suites[suiteName];
if (!suite) {
  throw new Error(`unknown suite ${suiteName}; expected one of ${Object.keys(suites).join(', ')}`);
}

const result = spawnSync(
  'hyperfine',
  [
    '--warmup',
    String(suite.warmup),
    '--runs',
    String(suite.runs),
    '--export-json',
    outputPath,
    ...suite.variants.flatMap(({ name, command }) => ['--command-name', name, command]),
  ],
  { cwd: import.meta.dirname, stdio: 'inherit' },
);

if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
