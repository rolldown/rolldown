// In-process single rolldown build so profilers and `/usr/bin/time` observe
// the actual work instead of a CLI fork. Pattern proven by the untracked
// rolldown-benchmark/apps/10000/rolldown-bench-direct.mjs.
//
// Usage:
//   NAPI_RS_NATIVE_LIBRARY_PATH=/tmp/bench-tokio.node \
//   FIXTURE=/abs/path/to/rolldown-benchmark/apps/1000 \
//   node direct.mjs
//
// The native binding is selected per-process via NAPI_RS_NATIVE_LIBRARY_PATH
// (honored first by the napi-rs loader in packages/rolldown/dist — no file
// swapping); the JS glue always comes from this repo's packages/rolldown/dist.
import { pathToFileURL } from 'node:url';
import path from 'node:path';

const fixture = process.env.FIXTURE;
if (!fixture) {
  throw new Error('set FIXTURE=/abs/path/to/rolldown-benchmark/apps/NNNN');
}
process.chdir(fixture);

const { rolldown } = await import(
  pathToFileURL(path.resolve(import.meta.dirname, '../../../packages/rolldown/dist/index.mjs'))
);

// Fixture configs export `defineConfig({ ... })` (an identity wrapper) with a
// single config object carrying an `output` object. Tolerate arrays on both
// levels; strip `output` from the input options passed to rolldown().
const configModule = await import(pathToFileURL(path.join(fixture, 'rolldown.config.mjs')));
let config = configModule.default;
if (Array.isArray(config)) config = config[0];
const { output: rawOutput, ...inputOptions } = config;
const output = (Array.isArray(rawOutput) ? rawOutput[0] : rawOutput) ?? {};

const t0 = performance.now();
const bundle = await rolldown(inputOptions);
await bundle.write(output);
const t1 = performance.now();
await bundle.close();
console.log(JSON.stringify({ fixture: path.basename(fixture), ms: +(t1 - t0).toFixed(1) }));
